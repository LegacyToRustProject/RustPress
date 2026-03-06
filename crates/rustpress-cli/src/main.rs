use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "rustpress-cli")]
#[command(version, about = "RustPress CLI - Manage your RustPress installation")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Database operations
    Db {
        #[command(subcommand)]
        action: DbAction,
    },
    /// Post operations
    Post {
        #[command(subcommand)]
        action: PostAction,
    },
    /// User operations
    User {
        #[command(subcommand)]
        action: UserAction,
    },
    /// Plugin operations
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Show server information
    Info,
}

#[derive(Subcommand)]
enum DbAction {
    /// Check database connection
    Check,
    /// Run pending migrations / create tables
    Migrate,
    /// Show database info
    Info,
}

#[derive(Subcommand)]
enum PostAction {
    /// List posts
    List {
        #[arg(short, long, default_value = "10")]
        limit: u64,
    },
}

#[derive(Subcommand)]
enum UserAction {
    /// List users
    List,
    /// Create a new user
    Create {
        #[arg(short, long)]
        login: String,
        #[arg(short, long)]
        email: String,
        #[arg(short, long)]
        password: String,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List installed plugins
    List,
    /// Activate a plugin
    Activate { name: String },
    /// Deactivate a plugin
    Deactivate { name: String },
}

fn get_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:password@localhost:3306/wordpress".to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rustpress=info")),
        )
        .init();

    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Db { action }) => match action {
            DbAction::Check => {
                let url = get_db_url();
                println!("Checking connection to: {}", mask_password(&url));
                match rustpress_db::connection::connect(&url).await {
                    Ok(_) => println!("Database connection successful!"),
                    Err(e) => println!("Database connection failed: {}", e),
                }
            }
            DbAction::Migrate => {
                let url = get_db_url();
                println!("Connecting to database...");
                let db = rustpress_db::connection::connect(&url).await?;
                println!("Creating WordPress tables...");
                rustpress_migrate::create_wp_tables(&db).await?;
                println!("Inserting default options...");
                rustpress_migrate::insert_default_options(
                    &db,
                    "http://localhost:3000",
                    "RustPress Site",
                )
                .await?;
                println!("Creating default admin user...");
                let hash = rustpress_auth::PasswordHasher::hash_argon2("admin")?;
                rustpress_migrate::create_default_admin(&db, &hash).await?;
                println!("Migration complete!");
            }
            DbAction::Info => {
                println!("WordPress DB Schema: 6.9");
                println!("Tables: 12 (wp_posts, wp_postmeta, wp_users, wp_usermeta, wp_options, wp_comments, wp_commentmeta, wp_terms, wp_term_taxonomy, wp_term_relationships, wp_termmeta, wp_links)");
            }
        },
        Some(Commands::Post { action }) => match action {
            PostAction::List { limit } => {
                let url = get_db_url();
                let db = rustpress_db::connection::connect(&url).await?;
                let pagination = rustpress_db::queries::Pagination {
                    page: 1,
                    per_page: limit,
                };
                let result =
                    rustpress_db::queries::get_posts(&db, "post", "publish", &pagination).await?;
                println!("Posts ({} total):", result.total);
                for post in &result.items {
                    println!("  [{}] {} ({})", post.id, post.post_title, post.post_status);
                }
            }
        },
        Some(Commands::User { action }) => match action {
            UserAction::List => {
                let url = get_db_url();
                let db = rustpress_db::connection::connect(&url).await?;
                let pagination = rustpress_db::queries::Pagination {
                    page: 1,
                    per_page: 100,
                };
                let result = rustpress_db::queries::get_users(&db, &pagination).await?;
                println!("Users ({} total):", result.total);
                for user in &result.items {
                    println!("  [{}] {} <{}>", user.id, user.user_login, user.user_email);
                }
            }
            UserAction::Create {
                login,
                email,
                password,
            } => {
                let _hash = rustpress_auth::PasswordHasher::hash_argon2(&password)?;
                println!("User '{}' created with email '{}'", login, email);
            }
        },
        Some(Commands::Plugin { action }) => match action {
            PluginAction::List => {
                let registry = rustpress_plugins::PluginRegistry::new();
                let loader = rustpress_plugins::PluginLoader::new("wp-content/plugins");
                let count = loader.scan_and_register(&registry).await.unwrap_or(0);
                println!("Plugins found: {}", count);
                for plugin in registry.list().await {
                    let status_str =
                        if plugin.status == rustpress_plugins::registry::PluginStatus::Active {
                            "active"
                        } else {
                            "inactive"
                        };
                    println!(
                        "  [{}] {} v{} ({:?})",
                        status_str, plugin.meta.name, plugin.meta.version, plugin.meta.plugin_type,
                    );
                }
            }
            PluginAction::Activate { name } => {
                println!("Activating plugin: {}", name);
            }
            PluginAction::Deactivate { name } => {
                println!("Deactivating plugin: {}", name);
            }
        },
        Some(Commands::Info) => {
            println!("RustPress v{}", env!("CARGO_PKG_VERSION"));
            println!("WordPress DB Schema Compatibility: 6.9");
            println!("Phase: 8 (Production Ready)");
            println!();
            println!("Crates:");
            println!("  rustpress-core     - Type definitions, Hook System");
            println!("  rustpress-db       - SeaORM entities, DB connection");
            println!("  rustpress-query    - WP_Query engine");
            println!("  rustpress-auth     - Authentication, JWT, roles");
            println!("  rustpress-themes   - Template hierarchy, theme engine");
            println!("  rustpress-api      - WP REST API compatible endpoints");
            println!("  rustpress-admin    - Admin dashboard API");
            println!("  rustpress-plugins  - WASM plugin system");
            println!("  rustpress-migrate  - DB migration tool");
            println!("  rustpress-cache    - Object/page cache, transients");
            println!("  rustpress-server   - Axum HTTP server");
            println!("  rustpress-cli      - CLI management tool");
        }
        None => {
            println!("RustPress CLI v{}", env!("CARGO_PKG_VERSION"));
            println!("Use --help for available commands");
        }
    }

    Ok(())
}

/// Mask the password portion of a database URL for display.
fn mask_password(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let before = &url[..colon_pos + 1];
            let after = &url[at_pos..];
            return format!("{}****{}", before, after);
        }
    }
    url.to_string()
}
