-- WordPress reference DB (used by the official WordPress Docker container)
CREATE DATABASE IF NOT EXISTS wordpress_ref CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
GRANT ALL PRIVILEGES ON wordpress_ref.* TO 'wpuser'@'%';

-- RustPress DB (reuses the 'wordpress' DB created by MYSQL_DATABASE env var)
-- Nothing to do here; 'wordpress' is already created by Docker env.

FLUSH PRIVILEGES;
