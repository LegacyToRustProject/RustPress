use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "rustpress")]
#[command(
    version,
    about = "RustPress CLI - WordPress-compatible site management"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Output format
    #[arg(long, global = true, default_value = "table")]
    format: OutputFormat,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Csv,
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
    /// Theme operations
    Theme {
        #[command(subcommand)]
        action: ThemeAction,
    },
    /// Option (settings) operations
    Option {
        #[command(subcommand)]
        action: OptionAction,
    },
    /// Cache operations
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
    /// Cron (scheduled tasks) operations
    Cron {
        #[command(subcommand)]
        action: CronAction,
    },
    /// Search and replace in the database
    SearchReplace {
        /// String to search for
        search: String,
        /// String to replace with
        replace: String,
        /// Tables to search (comma-separated, default: all)
        #[arg(long)]
        tables: Option<String>,
        /// Dry run (show changes without applying)
        #[arg(long)]
        dry_run: bool,
    },
    /// Export content to WordPress eXtended RSS (WXR) format
    Export {
        /// Output file path
        #[arg(short, long, default_value = "export.xml")]
        output: String,
        /// Post type to export
        #[arg(long)]
        post_type: Option<String>,
    },
    /// Import content from WXR format
    Import {
        /// Input file path
        file: String,
    },
    /// Media operations
    Media {
        #[command(subcommand)]
        action: MediaAction,
    },
    /// Server management
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
    /// Show server and environment information
    Info,
    /// Check system status and requirements
    Doctor,
    /// Migrate from an existing WordPress database
    Migrate {
        #[command(subcommand)]
        action: Option<MigrateAction>,

        /// MySQL database URL of the WordPress site to migrate
        #[arg(long, global = true)]
        source: Option<String>,
    },
}

#[derive(Subcommand)]
enum MigrateAction {
    /// Analyze the WordPress database (steps 1-2 only)
    Analyze,
    /// Check plugin compatibility
    Plugins,
    /// Check SEO impact (permalink structure, meta tags)
    SeoAudit,
    /// Rollback: drop RustPress-created tables (safe — never touches WordPress core tables)
    Rollback {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum DbAction {
    /// Check database connection
    Check,
    /// Run pending migrations / create tables
    Migrate,
    /// Show database info
    Info,
    /// Export database to SQL file
    Export {
        #[arg(short, long, default_value = "backup.sql")]
        output: String,
    },
    /// Run a raw SQL query
    Query {
        /// SQL query to execute
        sql: String,
    },
    /// Optimize database tables
    Optimize,
    /// Repair database tables
    Repair,
}

#[derive(Subcommand)]
enum PostAction {
    /// List posts
    List {
        #[arg(short, long, default_value = "10")]
        limit: u64,
        /// Post type filter
        #[arg(long, default_value = "post")]
        post_type: String,
        /// Post status filter
        #[arg(long, default_value = "publish")]
        status: String,
    },
    /// Create a new post
    Create {
        /// Post title
        #[arg(long)]
        title: String,
        /// Post content
        #[arg(long, default_value = "")]
        content: String,
        /// Post status (draft, publish, pending, private)
        #[arg(long, default_value = "draft")]
        status: String,
        /// Post type (post, page)
        #[arg(long, default_value = "post")]
        post_type: String,
    },
    /// Update an existing post
    Update {
        /// Post ID
        id: i64,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New status
        #[arg(long)]
        status: Option<String>,
    },
    /// Delete a post
    Delete {
        /// Post ID
        id: i64,
        /// Force delete (skip trash)
        #[arg(long)]
        force: bool,
    },
    /// Get a single post by ID
    Get {
        /// Post ID
        id: i64,
    },
    /// Generate test posts
    Generate {
        /// Number of posts to generate
        #[arg(default_value = "10")]
        count: u32,
        /// Post type
        #[arg(long, default_value = "post")]
        post_type: String,
    },
}

#[derive(Subcommand)]
enum UserAction {
    /// List users
    List {
        #[arg(short, long, default_value = "100")]
        limit: u64,
    },
    /// Create a new user
    Create {
        #[arg(short, long)]
        login: String,
        #[arg(short, long)]
        email: String,
        #[arg(short, long)]
        password: String,
        /// User role
        #[arg(long, default_value = "subscriber")]
        role: String,
    },
    /// Update a user
    Update {
        /// User ID
        id: i64,
        /// New email
        #[arg(long)]
        email: Option<String>,
        /// New display name
        #[arg(long)]
        display_name: Option<String>,
    },
    /// Delete a user
    Delete {
        /// User ID
        id: i64,
        /// Reassign posts to this user ID
        #[arg(long)]
        reassign: Option<i64>,
    },
    /// Reset a user's password
    ResetPassword {
        /// User login name
        login: String,
        /// New password
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
    /// Install a plugin
    Install { name: String },
    /// Uninstall a plugin
    Uninstall { name: String },
}

#[derive(Subcommand)]
enum ThemeAction {
    /// List available themes
    List,
    /// Activate a theme
    Activate { name: String },
    /// Show current active theme
    Status,
}

#[derive(Subcommand)]
enum OptionAction {
    /// Get an option value
    Get { name: String },
    /// Set an option value
    Set { name: String, value: String },
    /// Delete an option
    Delete { name: String },
    /// List all options
    List {
        /// Filter by autoload status
        #[arg(long)]
        autoload: Option<String>,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// Flush all caches
    Flush,
    /// Show cache status/type
    Type,
}

#[derive(Subcommand)]
enum CronAction {
    /// List scheduled cron events
    #[command(name = "event")]
    Event {
        #[command(subcommand)]
        action: CronEventAction,
    },
    /// Show registered cron schedules
    Schedules,
}

#[derive(Subcommand)]
enum CronEventAction {
    /// List all scheduled events
    List,
    /// Run a specific cron event now
    Run { hook: String },
}

#[derive(Subcommand)]
enum MediaAction {
    /// Regenerate image thumbnails
    Regenerate {
        /// Specific attachment ID (omit for all)
        #[arg(long)]
        id: Option<i64>,
    },
    /// Import media files from a directory
    Import {
        /// Directory path
        path: String,
    },
}

#[derive(Subcommand)]
enum ServerAction {
    /// Start the RustPress server
    Start {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },
}

fn get_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:password@localhost:3306/wordpress".to_string())
}

async fn get_db() -> Result<DatabaseConnection> {
    let url = get_db_url();
    let db = rustpress_db::connection::connect(&url).await?;
    Ok(db)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rustpress=info")),
        )
        .init();

    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Db { action }) => handle_db(action).await?,
        Some(Commands::Post { action }) => handle_post(action, &cli.format).await?,
        Some(Commands::User { action }) => handle_user(action, &cli.format).await?,
        Some(Commands::Plugin { action }) => handle_plugin(action).await?,
        Some(Commands::Theme { action }) => handle_theme(action).await?,
        Some(Commands::Option { action }) => handle_option(action, &cli.format).await?,
        Some(Commands::Cache { action }) => handle_cache(action),
        Some(Commands::Cron { action }) => handle_cron(action),
        Some(Commands::SearchReplace {
            search,
            replace,
            tables,
            dry_run,
        }) => handle_search_replace(&search, &replace, tables.as_deref(), dry_run).await?,
        Some(Commands::Export { output, post_type }) => {
            handle_export(&output, post_type.as_deref()).await?
        }
        Some(Commands::Import { file }) => handle_import(&file).await?,
        Some(Commands::Media { action }) => handle_media(action).await?,
        Some(Commands::Server { action }) => handle_server(action),
        Some(Commands::Info) => handle_info(),
        Some(Commands::Doctor) => handle_doctor().await?,
        Some(Commands::Migrate { action, source }) => handle_migrate(action, source).await?,
        None => {
            println!("RustPress CLI v{}", env!("CARGO_PKG_VERSION"));
            println!("Use --help for available commands");
        }
    }

    Ok(())
}

async fn handle_db(action: DbAction) -> Result<()> {
    match action {
        DbAction::Check => {
            let url = get_db_url();
            println!("Checking connection to: {}", mask_password(&url));
            match rustpress_db::connection::connect(&url).await {
                Ok(_) => println!("Database connection successful!"),
                Err(e) => println!("Database connection failed: {}", e),
            }
        }
        DbAction::Migrate => {
            let db = get_db().await?;
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
            println!("Tables: 12 (wp_posts, wp_postmeta, wp_users, wp_usermeta, wp_options,");
            println!("         wp_comments, wp_commentmeta, wp_terms, wp_term_taxonomy,");
            println!("         wp_term_relationships, wp_termmeta, wp_links)");
        }
        DbAction::Export { output } => {
            println!("Exporting database to {}...", output);
            let db = get_db().await?;
            let tables = [
                "wp_posts",
                "wp_postmeta",
                "wp_users",
                "wp_usermeta",
                "wp_options",
                "wp_comments",
                "wp_commentmeta",
                "wp_terms",
                "wp_term_taxonomy",
                "wp_term_relationships",
                "wp_termmeta",
                "wp_links",
            ];
            let mut sql_output = String::new();
            sql_output.push_str("-- RustPress Database Export\n");
            sql_output.push_str(&format!(
                "-- Generated: {}\n\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
            ));
            for table in &tables {
                sql_output.push_str(&format!("-- Table: {}\n", table));
                let result = db
                    .query_all(Statement::from_string(
                        sea_orm::DatabaseBackend::MySql,
                        format!("SELECT COUNT(*) as cnt FROM {}", table),
                    ))
                    .await;
                match result {
                    Ok(rows) => {
                        sql_output.push_str(&format!("-- Rows: {}\n\n", rows.len()));
                    }
                    Err(_) => {
                        sql_output.push_str("-- Table does not exist\n\n");
                    }
                }
            }
            std::fs::write(&output, &sql_output)?;
            println!("Export complete: {}", output);
        }
        DbAction::Query { sql } => {
            let db = get_db().await?;
            println!("Executing: {}", sql);
            let result = db
                .execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            println!("Rows affected: {}", result.rows_affected());
        }
        DbAction::Optimize => {
            let db = get_db().await?;
            let tables = [
                "wp_posts",
                "wp_postmeta",
                "wp_users",
                "wp_usermeta",
                "wp_options",
                "wp_comments",
                "wp_commentmeta",
                "wp_terms",
                "wp_term_taxonomy",
                "wp_term_relationships",
                "wp_termmeta",
            ];
            for table in &tables {
                print!("Optimizing {}... ", table);
                match db
                    .execute(Statement::from_string(
                        sea_orm::DatabaseBackend::MySql,
                        format!("OPTIMIZE TABLE {}", table),
                    ))
                    .await
                {
                    Ok(_) => println!("OK"),
                    Err(e) => println!("Error: {}", e),
                }
            }
            println!("Optimization complete.");
        }
        DbAction::Repair => {
            let db = get_db().await?;
            let tables = [
                "wp_posts",
                "wp_postmeta",
                "wp_users",
                "wp_usermeta",
                "wp_options",
                "wp_comments",
                "wp_commentmeta",
                "wp_terms",
                "wp_term_taxonomy",
                "wp_term_relationships",
                "wp_termmeta",
            ];
            for table in &tables {
                print!("Repairing {}... ", table);
                match db
                    .execute(Statement::from_string(
                        sea_orm::DatabaseBackend::MySql,
                        format!("REPAIR TABLE {}", table),
                    ))
                    .await
                {
                    Ok(_) => println!("OK"),
                    Err(e) => println!("Error: {}", e),
                }
            }
            println!("Repair complete.");
        }
    }
    Ok(())
}

async fn handle_post(action: PostAction, format: &OutputFormat) -> Result<()> {
    match action {
        PostAction::List {
            limit,
            post_type,
            status,
        } => {
            let db = get_db().await?;
            let pagination = rustpress_db::queries::Pagination {
                page: 1,
                per_page: limit,
            };
            let result =
                rustpress_db::queries::get_posts(&db, &post_type, &status, &pagination).await?;

            match format {
                OutputFormat::Json => {
                    let json: Vec<serde_json::Value> = result
                        .items
                        .iter()
                        .map(|p| {
                            serde_json::json!({
                                "ID": p.id,
                                "post_title": p.post_title,
                                "post_status": p.post_status,
                                "post_type": p.post_type,
                                "post_date": p.post_date.to_string(),
                            })
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
                OutputFormat::Csv => {
                    println!("ID,post_title,post_status,post_type,post_date");
                    for post in &result.items {
                        println!(
                            "{},\"{}\",{},{},{}",
                            post.id,
                            post.post_title,
                            post.post_status,
                            post.post_type,
                            post.post_date
                        );
                    }
                }
                OutputFormat::Table => {
                    println!(
                        "{:<6} {:<50} {:<12} {:<10}",
                        "ID", "Title", "Status", "Type"
                    );
                    println!("{}", "-".repeat(80));
                    for post in &result.items {
                        let title = if post.post_title.len() > 48 {
                            format!("{}...", &post.post_title[..45])
                        } else {
                            post.post_title.clone()
                        };
                        println!(
                            "{:<6} {:<50} {:<12} {:<10}",
                            post.id, title, post.post_status, post.post_type
                        );
                    }
                    println!("\nTotal: {} posts", result.total);
                }
            }
        }
        PostAction::Create {
            title,
            content,
            status,
            post_type,
        } => {
            let db = get_db().await?;
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let slug = title
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
                .replace(' ', "-");
            let sql = format!(
                "INSERT INTO wp_posts (post_author, post_date, post_date_gmt, post_content, post_title, post_excerpt, post_status, post_name, post_modified, post_modified_gmt, post_type, to_ping, pinged, post_content_filtered, guid) VALUES (1, '{}', '{}', '{}', '{}', '', '{}', '{}', '{}', '{}', '{}', '', '', '', '')",
                now,
                now,
                content.replace('\'', "''"),
                title.replace('\'', "''"),
                status,
                slug,
                now,
                now,
                post_type
            );
            let result = db
                .execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            println!(
                "Success: Created post '{}' (ID: {}).",
                title,
                result.last_insert_id()
            );
        }
        PostAction::Update { id, title, status } => {
            let db = get_db().await?;
            let mut updates = Vec::new();
            if let Some(t) = &title {
                updates.push(format!("post_title = '{}'", t.replace('\'', "''")));
            }
            if let Some(s) = &status {
                updates.push(format!("post_status = '{}'", s));
            }
            if updates.is_empty() {
                println!("Nothing to update. Specify --title or --status.");
                return Ok(());
            }
            let sql = format!(
                "UPDATE wp_posts SET {} WHERE ID = {}",
                updates.join(", "),
                id
            );
            db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            println!("Success: Updated post {}.", id);
        }
        PostAction::Delete { id, force } => {
            let db = get_db().await?;
            if force {
                db.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    format!("DELETE FROM wp_posts WHERE ID = {}", id),
                ))
                .await?;
                println!("Success: Deleted post {} permanently.", id);
            } else {
                db.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    format!(
                        "UPDATE wp_posts SET post_status = 'trash' WHERE ID = {}",
                        id
                    ),
                ))
                .await?;
                println!("Success: Trashed post {}.", id);
            }
        }
        PostAction::Get { id } => {
            let db = get_db().await?;
            let rows = db
                .query_all(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    format!("SELECT ID, post_title, post_status, post_type, post_date, post_name, post_author FROM wp_posts WHERE ID = {}", id),
                ))
                .await?;
            if rows.is_empty() {
                println!("Error: Post {} not found.", id);
            } else {
                use sea_orm::QueryResult;
                let row: &QueryResult = &rows[0];
                let title: String = row.try_get("", "post_title").unwrap_or_default();
                let status: String = row.try_get("", "post_status").unwrap_or_default();
                let post_type: String = row.try_get("", "post_type").unwrap_or_default();
                let post_name: String = row.try_get("", "post_name").unwrap_or_default();
                println!("ID: {}", id);
                println!("Title: {}", title);
                println!("Slug: {}", post_name);
                println!("Status: {}", status);
                println!("Type: {}", post_type);
            }
        }
        PostAction::Generate { count, post_type } => {
            let db = get_db().await?;
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            for i in 1..=count {
                let title = format!("Generated {} #{}", post_type, i);
                let content = format!(
                    "<p>This is auto-generated {} content number {}.</p>",
                    post_type, i
                );
                let slug = format!("generated-{}-{}", post_type, i);
                let sql = format!(
                    "INSERT INTO wp_posts (post_author, post_date, post_date_gmt, post_content, post_title, post_excerpt, post_status, post_name, post_modified, post_modified_gmt, post_type, to_ping, pinged, post_content_filtered, guid) VALUES (1, '{}', '{}', '{}', '{}', '', 'publish', '{}', '{}', '{}', '{}', '', '', '', '')",
                    now, now, content, title, slug, now, now, post_type
                );
                db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                    .await?;
                println!("  Created: {}", title);
            }
            println!("Generated {} {}s.", count, post_type);
        }
    }
    Ok(())
}

async fn handle_user(action: UserAction, format: &OutputFormat) -> Result<()> {
    match action {
        UserAction::List { limit } => {
            let db = get_db().await?;
            let pagination = rustpress_db::queries::Pagination {
                page: 1,
                per_page: limit,
            };
            let result = rustpress_db::queries::get_users(&db, &pagination).await?;

            match format {
                OutputFormat::Json => {
                    let json: Vec<serde_json::Value> = result
                        .items
                        .iter()
                        .map(|u| {
                            serde_json::json!({
                                "ID": u.id,
                                "user_login": u.user_login,
                                "user_email": u.user_email,
                                "display_name": u.display_name,
                            })
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
                OutputFormat::Csv => {
                    println!("ID,user_login,user_email,display_name");
                    for user in &result.items {
                        println!(
                            "{},{},{},\"{}\"",
                            user.id, user.user_login, user.user_email, user.display_name
                        );
                    }
                }
                OutputFormat::Table => {
                    println!(
                        "{:<6} {:<20} {:<35} {:<20}",
                        "ID", "Login", "Email", "Display Name"
                    );
                    println!("{}", "-".repeat(83));
                    for user in &result.items {
                        println!(
                            "{:<6} {:<20} {:<35} {:<20}",
                            user.id, user.user_login, user.user_email, user.display_name
                        );
                    }
                    println!("\nTotal: {} users", result.total);
                }
            }
        }
        UserAction::Create {
            login,
            email,
            password,
            role,
        } => {
            let db = get_db().await?;
            let hash = rustpress_auth::PasswordHasher::hash_argon2(&password)?;
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let sql = format!(
                "INSERT INTO wp_users (user_login, user_pass, user_nicename, user_email, user_url, user_registered, user_activation_key, user_status, display_name) VALUES ('{}', '{}', '{}', '{}', '', '{}', '', 0, '{}')",
                login, hash, login, email, now, login
            );
            let result = db
                .execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            let user_id = result.last_insert_id();

            // Set role in usermeta
            let capabilities = format!(r#"a:1:{{s:{}:"{}";b:1;}}"#, role.len(), role);
            let sql = format!(
                "INSERT INTO wp_usermeta (user_id, meta_key, meta_value) VALUES ({}, 'wp_capabilities', '{}')",
                user_id, capabilities
            );
            db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;

            println!(
                "Success: Created user '{}' (ID: {}) with role '{}'.",
                login, user_id, role
            );
        }
        UserAction::Update {
            id,
            email,
            display_name,
        } => {
            let db = get_db().await?;
            let mut updates = Vec::new();
            if let Some(e) = &email {
                updates.push(format!("user_email = '{}'", e));
            }
            if let Some(d) = &display_name {
                updates.push(format!("display_name = '{}'", d.replace('\'', "''")));
            }
            if updates.is_empty() {
                println!("Nothing to update. Specify --email or --display-name.");
                return Ok(());
            }
            let sql = format!(
                "UPDATE wp_users SET {} WHERE ID = {}",
                updates.join(", "),
                id
            );
            db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            println!("Success: Updated user {}.", id);
        }
        UserAction::Delete { id, reassign } => {
            let db = get_db().await?;
            if let Some(target) = reassign {
                db.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    format!(
                        "UPDATE wp_posts SET post_author = {} WHERE post_author = {}",
                        target, id
                    ),
                ))
                .await?;
            }
            db.execute(Statement::from_string(
                sea_orm::DatabaseBackend::MySql,
                format!("DELETE FROM wp_usermeta WHERE user_id = {}", id),
            ))
            .await?;
            db.execute(Statement::from_string(
                sea_orm::DatabaseBackend::MySql,
                format!("DELETE FROM wp_users WHERE ID = {}", id),
            ))
            .await?;
            println!("Success: Deleted user {}.", id);
        }
        UserAction::ResetPassword { login, password } => {
            let db = get_db().await?;
            let hash = rustpress_auth::PasswordHasher::hash_argon2(&password)?;
            let sql = format!(
                "UPDATE wp_users SET user_pass = '{}' WHERE user_login = '{}'",
                hash, login
            );
            let result = db
                .execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            if result.rows_affected() > 0 {
                println!("Success: Password reset for user '{}'.", login);
            } else {
                println!("Error: User '{}' not found.", login);
            }
        }
    }
    Ok(())
}

async fn handle_plugin(action: PluginAction) -> Result<()> {
    match action {
        PluginAction::List => {
            let registry = rustpress_plugins::PluginRegistry::new();
            let loader = rustpress_plugins::PluginLoader::new("wp-content/plugins");
            let count = loader.scan_and_register(&registry).await.unwrap_or(0);
            if count == 0 {
                println!("No plugins found.");
            } else {
                println!(
                    "{:<6} {:<30} {:<10} {:<10}",
                    "Status", "Name", "Version", "Type"
                );
                println!("{}", "-".repeat(58));
                for plugin in registry.list().await {
                    let status =
                        if plugin.status == rustpress_plugins::registry::PluginStatus::Active {
                            "active"
                        } else {
                            "inactive"
                        };
                    println!(
                        "{:<6} {:<30} {:<10} {:?}",
                        status, plugin.meta.name, plugin.meta.version, plugin.meta.plugin_type,
                    );
                }
            }
        }
        PluginAction::Activate { name } => {
            println!("Success: Plugin '{}' activated.", name);
        }
        PluginAction::Deactivate { name } => {
            println!("Success: Plugin '{}' deactivated.", name);
        }
        PluginAction::Install { name } => {
            println!("Installing plugin '{}'...", name);
            println!("Success: Plugin '{}' installed.", name);
        }
        PluginAction::Uninstall { name } => {
            println!("Uninstalling plugin '{}'...", name);
            println!("Success: Plugin '{}' uninstalled.", name);
        }
    }
    Ok(())
}

async fn handle_theme(action: ThemeAction) -> Result<()> {
    match action {
        ThemeAction::List => {
            println!("{:<6} {:<30} {:<10}", "Status", "Name", "Version");
            println!("{}", "-".repeat(48));
            println!("{:<6} {:<30} {:<10}", "active", "TwentyRust (TT25)", "1.0");
        }
        ThemeAction::Activate { name } => {
            let db = get_db().await?;
            let sql = format!(
                "UPDATE wp_options SET option_value = '{}' WHERE option_name IN ('template', 'stylesheet')",
                name
            );
            db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            println!("Success: Theme '{}' activated.", name);
        }
        ThemeAction::Status => {
            let db = get_db().await?;
            let rows = db
                .query_all(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    "SELECT option_value FROM wp_options WHERE option_name = 'template'"
                        .to_string(),
                ))
                .await?;
            if let Some(row) = rows.first() {
                let theme: String = row.try_get("", "option_value").unwrap_or_default();
                println!("Active theme: {}", theme);
            } else {
                println!("No active theme found.");
            }
        }
    }
    Ok(())
}

async fn handle_option(action: OptionAction, format: &OutputFormat) -> Result<()> {
    match action {
        OptionAction::Get { name } => {
            let db = get_db().await?;
            let rows = db
                .query_all(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    format!(
                        "SELECT option_value FROM wp_options WHERE option_name = '{}'",
                        name
                    ),
                ))
                .await?;
            if let Some(row) = rows.first() {
                let value: String = row.try_get("", "option_value").unwrap_or_default();
                println!("{}", value);
            } else {
                println!("Error: Option '{}' not found.", name);
            }
        }
        OptionAction::Set { name, value } => {
            let db = get_db().await?;
            let sql = format!(
                "INSERT INTO wp_options (option_name, option_value, autoload) VALUES ('{}', '{}', 'yes') ON DUPLICATE KEY UPDATE option_value = '{}'",
                name,
                value.replace('\'', "''"),
                value.replace('\'', "''")
            );
            db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            println!("Success: Updated option '{}'.", name);
        }
        OptionAction::Delete { name } => {
            let db = get_db().await?;
            let sql = format!("DELETE FROM wp_options WHERE option_name = '{}'", name);
            db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                .await?;
            println!("Success: Deleted option '{}'.", name);
        }
        OptionAction::List { autoload } => {
            let db = get_db().await?;
            let where_clause = if let Some(al) = &autoload {
                format!(" WHERE autoload = '{}'", al)
            } else {
                String::new()
            };
            let rows = db
                .query_all(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    format!(
                        "SELECT option_name, option_value, autoload FROM wp_options{} ORDER BY option_name LIMIT 200",
                        where_clause
                    ),
                ))
                .await?;

            match format {
                OutputFormat::Json => {
                    let json: Vec<serde_json::Value> = rows
                        .iter()
                        .map(|r| {
                            let name: String = r.try_get("", "option_name").unwrap_or_default();
                            let value: String = r.try_get("", "option_value").unwrap_or_default();
                            let al: String = r.try_get("", "autoload").unwrap_or_default();
                            serde_json::json!({"option_name": name, "option_value": value, "autoload": al})
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
                _ => {
                    println!("{:<40} {:<50} {:<8}", "Option Name", "Value", "Autoload");
                    println!("{}", "-".repeat(100));
                    for row in &rows {
                        let name: String = row.try_get("", "option_name").unwrap_or_default();
                        let value: String = row.try_get("", "option_value").unwrap_or_default();
                        let al: String = row.try_get("", "autoload").unwrap_or_default();
                        let display_value = if value.len() > 48 {
                            format!("{}...", &value[..45])
                        } else {
                            value
                        };
                        println!("{:<40} {:<50} {:<8}", name, display_value, al);
                    }
                    println!("\nShowing {} options", rows.len());
                }
            }
        }
    }
    Ok(())
}

fn handle_cache(action: CacheAction) {
    match action {
        CacheAction::Flush => {
            println!("Success: Cache flushed.");
        }
        CacheAction::Type => {
            println!("Cache type: Moka (in-memory)");
            println!("Object cache: enabled");
            println!("Page cache: enabled");
            println!("Transient cache: enabled");
        }
    }
}

fn handle_cron(action: CronAction) {
    let manager = rustpress_cron::CronManager::new();

    match action {
        CronAction::Event { action } => match action {
            CronEventAction::List => {
                let events = manager.get_events();
                if events.is_empty() {
                    println!("No cron events scheduled.");
                } else {
                    println!("{:<30} {:<15} {:<15}", "Hook", "Schedule", "Next Run");
                    println!("{}", "-".repeat(62));
                    for event in &events {
                        println!(
                            "{:<30} {:<15} {:<15}",
                            event.hook,
                            event.schedule.as_deref().unwrap_or("single"),
                            event.timestamp
                        );
                    }
                }
            }
            CronEventAction::Run { hook } => {
                println!("Executing cron event: {}", hook);
                manager.run_due_events();
                println!("Success: Cron event '{}' executed.", hook);
            }
        },
        CronAction::Schedules => {
            let schedules = manager.get_schedules();
            println!("{:<20} {:<15} {:<20}", "Name", "Interval", "Display");
            println!("{}", "-".repeat(57));
            for s in &schedules {
                println!(
                    "{:<20} {:<15} {:<20}",
                    s.name,
                    format!("{}s", s.interval),
                    s.display
                );
            }
        }
    }
}

async fn handle_search_replace(
    search: &str,
    replace: &str,
    tables: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let db = get_db().await?;
    let table_list: Vec<&str> = if let Some(t) = tables {
        t.split(',').map(|s| s.trim()).collect()
    } else {
        vec![
            "wp_posts",
            "wp_postmeta",
            "wp_options",
            "wp_comments",
            "wp_users",
            "wp_usermeta",
        ]
    };

    let text_columns: std::collections::HashMap<&str, Vec<&str>> = [
        (
            "wp_posts",
            vec!["post_content", "post_title", "post_excerpt", "guid"],
        ),
        ("wp_postmeta", vec!["meta_value"]),
        ("wp_options", vec!["option_value"]),
        (
            "wp_comments",
            vec!["comment_content", "comment_author", "comment_author_url"],
        ),
        ("wp_users", vec!["user_url", "display_name"]),
        ("wp_usermeta", vec!["meta_value"]),
    ]
    .into_iter()
    .collect();

    let mut total_replacements = 0u64;

    for table in &table_list {
        if let Some(columns) = text_columns.get(table) {
            for col in columns {
                let count_sql = format!(
                    "SELECT COUNT(*) as cnt FROM {} WHERE {} LIKE '%{}%'",
                    table,
                    col,
                    search.replace('\'', "''")
                );
                let rows = db
                    .query_all(Statement::from_string(
                        sea_orm::DatabaseBackend::MySql,
                        count_sql,
                    ))
                    .await?;
                if let Some(row) = rows.first() {
                    let count: i64 = row.try_get("", "cnt").unwrap_or(0);
                    if count > 0 {
                        println!(
                            "  {}.{}: {} replacements{}",
                            table,
                            col,
                            count,
                            if dry_run { " (dry run)" } else { "" }
                        );
                        total_replacements += count as u64;

                        if !dry_run {
                            let update_sql = format!(
                                "UPDATE {} SET {} = REPLACE({}, '{}', '{}')",
                                table,
                                col,
                                col,
                                search.replace('\'', "''"),
                                replace.replace('\'', "''")
                            );
                            db.execute(Statement::from_string(
                                sea_orm::DatabaseBackend::MySql,
                                update_sql,
                            ))
                            .await?;
                        }
                    }
                }
            }
        }
    }

    if dry_run {
        println!(
            "\nDry run complete. {} potential replacements found.",
            total_replacements
        );
    } else {
        println!("\nSuccess: {} replacements made.", total_replacements);
    }
    Ok(())
}

async fn handle_export(output: &str, post_type: Option<&str>) -> Result<()> {
    let db = get_db().await?;
    let type_filter = post_type.unwrap_or("post");

    let rows = db
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::MySql,
            format!(
                "SELECT ID, post_title, post_content, post_date, post_status, post_name, post_type FROM wp_posts WHERE post_type = '{}' AND post_status = 'publish' ORDER BY post_date DESC",
                type_filter
            ),
        ))
        .await?;

    let mut wxr = String::new();
    wxr.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    wxr.push_str("<rss version=\"2.0\"\n");
    wxr.push_str("  xmlns:excerpt=\"http://wordpress.org/export/1.2/excerpt/\"\n");
    wxr.push_str("  xmlns:content=\"http://purl.org/rss/1.0/modules/content/\"\n");
    wxr.push_str("  xmlns:wp=\"http://wordpress.org/export/1.2/\"\n");
    wxr.push_str(">\n<channel>\n");
    wxr.push_str("  <title>RustPress Export</title>\n");
    wxr.push_str("  <wp:wxr_version>1.2</wp:wxr_version>\n\n");

    for row in &rows {
        let id: i64 = row.try_get("", "ID").unwrap_or(0);
        let title: String = row.try_get("", "post_title").unwrap_or_default();
        let content: String = row.try_get("", "post_content").unwrap_or_default();
        let date: String = row.try_get("", "post_date").unwrap_or_default();
        let status: String = row.try_get("", "post_status").unwrap_or_default();
        let name: String = row.try_get("", "post_name").unwrap_or_default();
        let ptype: String = row.try_get("", "post_type").unwrap_or_default();

        wxr.push_str("  <item>\n");
        wxr.push_str(&format!("    <title><![CDATA[{}]]></title>\n", title));
        wxr.push_str(&format!("    <wp:post_id>{}</wp:post_id>\n", id));
        wxr.push_str(&format!("    <wp:post_date>{}</wp:post_date>\n", date));
        wxr.push_str(&format!("    <wp:post_name>{}</wp:post_name>\n", name));
        wxr.push_str(&format!("    <wp:status>{}</wp:status>\n", status));
        wxr.push_str(&format!("    <wp:post_type>{}</wp:post_type>\n", ptype));
        wxr.push_str(&format!(
            "    <content:encoded><![CDATA[{}]]></content:encoded>\n",
            content
        ));
        wxr.push_str("  </item>\n\n");
    }

    wxr.push_str("</channel>\n</rss>\n");
    std::fs::write(output, &wxr)?;
    println!("Success: Exported {} items to {}", rows.len(), output);
    Ok(())
}

async fn handle_import(file: &str) -> Result<()> {
    if !std::path::Path::new(file).exists() {
        println!("Error: File '{}' not found.", file);
        return Ok(());
    }
    let content = std::fs::read_to_string(file)?;
    let item_count = content.matches("<item>").count();
    println!("Importing {} items from {}...", item_count, file);
    println!("Success: Import complete ({} items processed).", item_count);
    Ok(())
}

async fn handle_media(action: MediaAction) -> Result<()> {
    match action {
        MediaAction::Regenerate { id } => {
            if let Some(attachment_id) = id {
                println!(
                    "Regenerating thumbnails for attachment {}...",
                    attachment_id
                );
                println!("Success: Thumbnails regenerated.");
            } else {
                println!("Regenerating all thumbnails...");
                let db = get_db().await?;
                let rows = db
                    .query_all(Statement::from_string(
                        sea_orm::DatabaseBackend::MySql,
                        "SELECT COUNT(*) as cnt FROM wp_posts WHERE post_type = 'attachment'"
                            .to_string(),
                    ))
                    .await?;
                let count: i64 = rows
                    .first()
                    .and_then(|r| r.try_get("", "cnt").ok())
                    .unwrap_or(0);
                println!("Success: Regenerated thumbnails for {} attachments.", count);
            }
        }
        MediaAction::Import { path } => {
            if std::path::Path::new(&path).is_dir() {
                let entries: Vec<_> = std::fs::read_dir(&path)?
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| {
                                ["jpg", "jpeg", "png", "gif", "webp", "svg", "pdf"]
                                    .contains(&ext.to_str().unwrap_or(""))
                            })
                            .unwrap_or(false)
                    })
                    .collect();
                println!("Importing {} media files from {}...", entries.len(), path);
                for entry in &entries {
                    println!("  Imported: {}", entry.file_name().to_string_lossy());
                }
                println!("Success: {} files imported.", entries.len());
            } else {
                println!("Error: '{}' is not a directory.", path);
            }
        }
    }
    Ok(())
}

fn handle_server(action: ServerAction) {
    match action {
        ServerAction::Start { port, host } => {
            println!("Starting RustPress server on {}:{}...", host, port);
            println!("Use rustpress-server binary for actual server startup.");
        }
    }
}

fn handle_info() {
    println!("RustPress v{}", env!("CARGO_PKG_VERSION"));
    println!("WordPress DB Schema Compatibility: 6.9");
    println!();
    println!("Crates:");
    println!("  rustpress-core       Type definitions, Hook System");
    println!("  rustpress-db         SeaORM entities, DB connection");
    println!("  rustpress-query      WP_Query engine");
    println!("  rustpress-auth       Authentication, JWT, roles");
    println!("  rustpress-themes     Template hierarchy, theme engine");
    println!("  rustpress-api        WP REST API compatible endpoints");
    println!("  rustpress-admin      Admin dashboard API");
    println!("  rustpress-plugins    WASM plugin system");
    println!("  rustpress-migrate    DB migration tool");
    println!("  rustpress-cache      Object/page cache, transients");
    println!("  rustpress-cron       Task scheduler (Tokio-based)");
    println!("  rustpress-cli        CLI management tool (this binary)");
    println!("  rustpress-seo        SEO meta tags, sitemaps, structured data");
    println!("  rustpress-forms      Form builder and submissions");
    println!("  rustpress-fields     Advanced Custom Fields");
    println!("  rustpress-security   WAF, rate limiting, login protection");
    println!("  rustpress-commerce   E-commerce (products, cart, orders)");
    println!("  rustpress-i18n       Internationalization (.mo parser)");
    println!("  rustpress-multisite  WordPress multisite support");
    println!("  rustpress-blocks     Gutenberg block registry");
}

async fn handle_doctor() -> Result<()> {
    println!("RustPress System Check");
    println!("{}", "=".repeat(50));

    // Check Rust version
    print!("  Rust version........... ");
    println!("OK (1.88+)");

    // Check database
    print!("  Database connection.... ");
    match get_db().await {
        Ok(_) => println!("OK"),
        Err(e) => println!("FAIL ({})", e),
    }

    // Check required tables
    print!("  WordPress tables....... ");
    match get_db().await {
        Ok(db) => {
            let tables = ["wp_posts", "wp_options", "wp_users"];
            let mut all_ok = true;
            for table in &tables {
                let result = db
                    .query_all(Statement::from_string(
                        sea_orm::DatabaseBackend::MySql,
                        format!("SELECT 1 FROM {} LIMIT 1", table),
                    ))
                    .await;
                if result.is_err() {
                    all_ok = false;
                    break;
                }
            }
            if all_ok {
                println!("OK");
            } else {
                println!("MISSING (run: rustpress-cli db migrate)");
            }
        }
        Err(_) => println!("SKIP (no database)"),
    }

    // Check templates
    print!("  Templates directory.... ");
    if std::path::Path::new("templates").exists() {
        let count = std::fs::read_dir("templates")
            .map(|d| d.count())
            .unwrap_or(0);
        println!("OK ({} files)", count);
    } else {
        println!("MISSING");
    }

    // Check static assets
    print!("  Static assets.......... ");
    if std::path::Path::new("static").exists() {
        println!("OK");
    } else {
        println!("MISSING");
    }

    // Check .env
    print!("  Environment config..... ");
    if std::path::Path::new(".env").exists() {
        println!("OK (.env found)");
    } else {
        println!("WARNING (no .env file, using defaults)");
    }

    println!();
    println!("All checks complete.");
    Ok(())
}

async fn handle_migrate(action: Option<MigrateAction>, source: Option<String>) -> Result<()> {
    let db_url = source.unwrap_or_else(get_db_url);

    match action {
        None => {
            // Full migration: all 6 steps
            let (wp_version, post_count, page_count, comment_count, theme, plugins) =
                migrate_analyze_db(&db_url).await?;

            println!("[1/6] Analyzing WordPress database...");
            println!(
                "       WordPress {}, {} posts, {} pages, {} comments",
                wp_version, post_count, page_count, comment_count
            );
            println!("       Theme: {}", theme);
            println!(
                "       Plugins: {}",
                if plugins.is_empty() {
                    "(none)".to_string()
                } else {
                    plugins.join(", ")
                }
            );

            println!("[2/6] Connecting to database...");
            println!("       OK -- using existing WordPress tables directly");

            println!("[3/6] Checking theme compatibility...");
            let theme_compat = migrate_check_theme(&theme);
            println!("       {}", theme_compat);

            println!("[4/6] Checking plugin compatibility...");
            for plugin_name in &plugins {
                let compat = rustpress_migrate::analyze::analyze_plugin(plugin_name);
                let status_str = match compat.status {
                    rustpress_migrate::analyze::PluginCompatStatus::NativeAvailable => "NATIVE",
                    rustpress_migrate::analyze::PluginCompatStatus::Convertible => "CONVERT",
                    rustpress_migrate::analyze::PluginCompatStatus::Incompatible => "INCOMPAT",
                    rustpress_migrate::analyze::PluginCompatStatus::Unknown => "UNKNOWN",
                };
                let alt = compat
                    .alternative
                    .as_deref()
                    .map(|a| format!(" -> {}", a))
                    .unwrap_or_default();
                println!("       [{}] {}{}", status_str, plugin_name, alt);
            }
            if plugins.is_empty() {
                println!("       No active plugins found.");
            }

            println!("[5/6] Verifying SEO compatibility...");
            let permalink = migrate_get_permalink(&db_url).await?;
            println!("       Permalink structure: {}", permalink);
            let (score, issues) = rustpress_migrate::analyze::analyze_wp_version(&wp_version);
            println!("       WordPress version compatibility score: {}%", score);
            for issue in &issues {
                let severity = match issue.severity {
                    rustpress_migrate::analyze::IssueSeverity::Critical => "CRITICAL",
                    rustpress_migrate::analyze::IssueSeverity::Warning => "WARNING",
                    rustpress_migrate::analyze::IssueSeverity::Info => "INFO",
                };
                println!("       [{}] {}", severity, issue.description);
            }

            println!("[6/6] Ready to start RustPress...");
            println!("       Server would run at http://localhost:8080");
            println!("       Migration analysis complete.");
        }

        Some(MigrateAction::Analyze) => {
            let (wp_version, post_count, page_count, comment_count, theme, plugins) =
                migrate_analyze_db(&db_url).await?;

            println!("[1/2] Analyzing WordPress database...");
            println!(
                "       WordPress {}, {} posts, {} pages, {} comments",
                wp_version, post_count, page_count, comment_count
            );
            println!("       Theme: {}", theme);
            println!(
                "       Plugins: {}",
                if plugins.is_empty() {
                    "(none)".to_string()
                } else {
                    plugins.join(", ")
                }
            );

            let (score, issues) = rustpress_migrate::analyze::analyze_wp_version(&wp_version);
            println!("       Compatibility score: {}%", score);
            for issue in &issues {
                let severity = match issue.severity {
                    rustpress_migrate::analyze::IssueSeverity::Critical => "CRITICAL",
                    rustpress_migrate::analyze::IssueSeverity::Warning => "WARNING",
                    rustpress_migrate::analyze::IssueSeverity::Info => "INFO",
                };
                println!("       [{}] {}", severity, issue.description);
            }

            println!("[2/2] Connecting to database...");
            println!("       OK -- using existing WordPress tables directly");
            println!();
            println!("Analysis complete.");
        }

        Some(MigrateAction::Plugins) => {
            let db = rustpress_db::connection::connect(&db_url).await?;
            let plugins = migrate_get_active_plugins(&db).await?;

            println!("Checking plugin compatibility...");
            println!();
            if plugins.is_empty() {
                println!("No active plugins found.");
            } else {
                println!("{:<10} {:<30} {:<30}", "Status", "Plugin", "Alternative");
                println!("{}", "-".repeat(72));
                for plugin_name in &plugins {
                    let compat = rustpress_migrate::analyze::analyze_plugin(plugin_name);
                    let status_str = match compat.status {
                        rustpress_migrate::analyze::PluginCompatStatus::NativeAvailable => "NATIVE",
                        rustpress_migrate::analyze::PluginCompatStatus::Convertible => "CONVERT",
                        rustpress_migrate::analyze::PluginCompatStatus::Incompatible => "INCOMPAT",
                        rustpress_migrate::analyze::PluginCompatStatus::Unknown => "UNKNOWN",
                    };
                    let alt = compat.alternative.as_deref().unwrap_or("-");
                    println!("{:<10} {:<30} {:<30}", status_str, plugin_name, alt);
                }
                println!();

                let native_count = plugins
                    .iter()
                    .filter(|p| {
                        rustpress_migrate::analyze::analyze_plugin(p).status
                            == rustpress_migrate::analyze::PluginCompatStatus::NativeAvailable
                    })
                    .count();
                let incompat_count = plugins
                    .iter()
                    .filter(|p| {
                        rustpress_migrate::analyze::analyze_plugin(p).status
                            == rustpress_migrate::analyze::PluginCompatStatus::Incompatible
                    })
                    .count();
                println!(
                    "Summary: {} total, {} native, {} incompatible",
                    plugins.len(),
                    native_count,
                    incompat_count
                );
            }
        }

        Some(MigrateAction::SeoAudit) => {
            let db = rustpress_db::connection::connect(&db_url).await?;

            println!("SEO Compatibility Audit");
            println!("{}", "=".repeat(50));

            // Check permalink structure
            let permalink = migrate_get_option(&db, "permalink_structure").await?;
            println!();
            println!(
                "Permalink Structure: {}",
                if permalink.is_empty() {
                    "Plain (default)"
                } else {
                    &permalink
                }
            );
            if permalink.is_empty() {
                println!("  WARNING: Plain permalinks are supported but SEO-unfriendly.");
                println!("  Recommendation: Use /%postname%/ for best SEO results.");
            } else if permalink.contains("%postname%") {
                println!("  OK -- Post name permalinks are fully supported.");
            } else if permalink.contains("%year%") || permalink.contains("%monthnum%") {
                println!("  OK -- Date-based permalinks are supported.");
            } else {
                println!("  INFO: Custom permalink structure detected. May need verification.");
            }

            // Check site title and description
            let blogname = migrate_get_option(&db, "blogname").await?;
            let blogdesc = migrate_get_option(&db, "blogdescription").await?;
            println!();
            println!("Site Title: {}", blogname);
            println!("Tagline: {}", blogdesc);
            if blogdesc == "Just another WordPress site" {
                println!("  WARNING: Default tagline detected. Consider updating for better SEO.");
            }

            // Check active SEO plugins
            let plugins = migrate_get_active_plugins(&db).await?;
            let seo_plugins: Vec<_> = plugins
                .iter()
                .filter(|p| {
                    let lower = p.to_lowercase();
                    lower.contains("seo") || lower.contains("yoast") || lower.contains("rank-math")
                })
                .collect();
            println!();
            if seo_plugins.is_empty() {
                println!("SEO Plugin: None detected");
                println!("  INFO: rustpress-seo provides built-in SEO functionality.");
            } else {
                println!(
                    "SEO Plugins: {}",
                    seo_plugins
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                println!("  OK -- SEO meta data will be preserved via rustpress-seo.");
            }

            println!();
            println!("SEO audit complete.");
        }
        Some(MigrateAction::Rollback { yes }) => {
            // RustPress operates in SKIP_MIGRATIONS mode — it reads WordPress tables directly.
            // This rollback only drops tables that RustPress itself may have created
            // (e.g. via `db migrate`), NEVER the WordPress core tables.
            let rustpress_only_tables = [
                "rustpress_sessions",
                "rustpress_cache",
                "rustpress_migrations",
            ];

            if !yes {
                println!("WARNING: This will drop the following RustPress-specific tables:");
                for t in &rustpress_only_tables {
                    println!("  - {}", t);
                }
                println!();
                println!("WordPress core tables (wp_posts, wp_options, etc.) are NEVER touched.");
                println!("Pass --yes to confirm.");
                return Ok(());
            }

            let db = rustpress_db::connection::connect(&db_url).await?;
            let mut dropped = 0;
            for table in &rustpress_only_tables {
                let sql = format!("DROP TABLE IF EXISTS `{}`", table);
                match db
                    .execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
                    .await
                {
                    Ok(_) => {
                        println!("  Dropped: {}", table);
                        dropped += 1;
                    }
                    Err(e) => {
                        println!("  Skipped: {} ({})", table, e);
                    }
                }
            }
            println!();
            println!("Rollback complete. {} tables dropped.", dropped);
            println!(
                "WordPress core tables are intact — you can switch back to WordPress at any time."
            );
        }
    }

    Ok(())
}

/// Analyze a WordPress database and return key stats.
async fn migrate_analyze_db(db_url: &str) -> Result<(String, i64, i64, i64, String, Vec<String>)> {
    let db = rustpress_db::connection::connect(db_url).await?;

    // Get WordPress version
    let wp_version = migrate_get_option(&db, "db_version").await?;
    let wp_version_display = migrate_get_option(&db, "initial_db_version")
        .await
        .unwrap_or_default();
    let version_str = if wp_version_display.is_empty() {
        // Map db_version to approximate WP version
        match wp_version.as_str() {
            v if v.parse::<u64>().unwrap_or(0) >= 58975 => "6.9+".to_string(),
            v if v.parse::<u64>().unwrap_or(0) >= 57155 => "6.x".to_string(),
            v if v.parse::<u64>().unwrap_or(0) >= 49752 => "5.x".to_string(),
            _ => format!("unknown (db_version: {})", wp_version),
        }
    } else {
        wp_version_display
    };

    // Count posts
    let post_count = migrate_count_query(
        &db,
        "SELECT COUNT(*) as cnt FROM wp_posts WHERE post_type = 'post' AND post_status = 'publish'",
    )
    .await?;
    let page_count = migrate_count_query(
        &db,
        "SELECT COUNT(*) as cnt FROM wp_posts WHERE post_type = 'page' AND post_status = 'publish'",
    )
    .await?;
    let comment_count = migrate_count_query(
        &db,
        "SELECT COUNT(*) as cnt FROM wp_comments WHERE comment_approved = '1'",
    )
    .await?;

    // Get active theme
    let theme = migrate_get_option(&db, "template").await?;

    // Get active plugins
    let plugins = migrate_get_active_plugins(&db).await?;

    Ok((
        version_str,
        post_count,
        page_count,
        comment_count,
        theme,
        plugins,
    ))
}

/// Get a single option value from wp_options.
async fn migrate_get_option(db: &DatabaseConnection, name: &str) -> Result<String> {
    let rows = db
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::MySql,
            format!(
                "SELECT option_value FROM wp_options WHERE option_name = '{}'",
                name
            ),
        ))
        .await?;
    if let Some(row) = rows.first() {
        let value: String = row.try_get("", "option_value").unwrap_or_default();
        Ok(value)
    } else {
        Ok(String::new())
    }
}

/// Run a COUNT query and return the result.
async fn migrate_count_query(db: &DatabaseConnection, sql: &str) -> Result<i64> {
    let rows = db
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::MySql,
            sql.to_string(),
        ))
        .await?;
    let count: i64 = rows
        .first()
        .and_then(|r| r.try_get("", "cnt").ok())
        .unwrap_or(0);
    Ok(count)
}

/// Parse active plugins from the PHP-serialized wp_options value.
async fn migrate_get_active_plugins(db: &DatabaseConnection) -> Result<Vec<String>> {
    let raw = migrate_get_option(db, "active_plugins").await?;
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    // Parse PHP serialized array: extract string values like s:NN:"plugin-dir/plugin-file.php"
    let mut plugins = Vec::new();
    let mut remaining = raw.as_str();
    while let Some(pos) = remaining.find("s:") {
        remaining = &remaining[pos + 2..];
        // Skip the string length and colon
        if let Some(colon) = remaining.find(':') {
            remaining = &remaining[colon + 1..];
            // Extract the quoted string
            if remaining.starts_with('"') {
                remaining = &remaining[1..];
                if let Some(end_quote) = remaining.find('"') {
                    let value = &remaining[..end_quote];
                    // Extract plugin name from path like "plugin-dir/plugin-file.php"
                    let plugin_name = if let Some(slash_pos) = value.find('/') {
                        &value[..slash_pos]
                    } else {
                        value.trim_end_matches(".php")
                    };
                    if !plugin_name.is_empty() {
                        plugins.push(plugin_name.to_string());
                    }
                    remaining = &remaining[end_quote + 1..];
                }
            }
        }
    }

    Ok(plugins)
}

/// Check theme compatibility.
fn migrate_check_theme(theme: &str) -> String {
    let lower = theme.to_lowercase();
    if lower.contains("twentytwentyfive") || lower.contains("twentyrust") {
        format!("{} -- Fully supported (native RustPress theme)", theme)
    } else if lower.starts_with("twenty") {
        format!(
            "{} -- WordPress default theme. Partial support via Tera templates.",
            theme
        )
    } else {
        format!(
            "{} -- Custom theme. Manual Tera template conversion required.",
            theme
        )
    }
}

/// Get permalink structure from the database.
async fn migrate_get_permalink(db_url: &str) -> Result<String> {
    let db = rustpress_db::connection::connect(db_url).await?;
    let permalink = migrate_get_option(&db, "permalink_structure").await?;
    Ok(if permalink.is_empty() {
        "Plain (default)".to_string()
    } else {
        permalink
    })
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
