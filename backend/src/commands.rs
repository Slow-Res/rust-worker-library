//! One-shot CLI commands that run instead of the server.

use std::io::BufRead;
use std::sync::Arc;

use anyhow::{Context, bail};

use crate::auth::{AuthService, SqliteAuthRepository};
use crate::clock::SystemClock;
use crate::config::Config;
use crate::db;
use crate::error::AppError;

/// `create-user`: add a librarian account to the database.
///
/// Errors (short password, mismatched confirmation, duplicate username) are
/// returned to `main`, which prints them and exits non-zero.
pub async fn create_user(
    config: Config,
    username: &str,
    password_stdin: bool,
) -> anyhow::Result<()> {
    // Read the password first so a typo fails before touching the database.
    // (Blocking on the terminal is fine here: nothing else is running yet.)
    let password = if password_stdin {
        read_password_from_stdin()?
    } else {
        prompt_password()?
    };

    let pool = db::connect(&config).await?;
    db::migrate(&pool).await?;
    let auth = AuthService::new(
        Arc::new(SqliteAuthRepository::new(pool.clone())),
        Arc::new(SystemClock),
        chrono::Duration::hours(config.session_ttl_hours),
    );

    let result = auth.create_user(username, &password).await;
    pool.close().await;
    // `AppError`'s message is user-facing for validation and conflict errors.
    let user = result.map_err(|err| anyhow::anyhow!("{}", describe(&err)))?;
    println!("Created user `{}` (id {})", user.username, user.id);
    Ok(())
}

fn describe(err: &AppError) -> String {
    match err {
        AppError::Validation(fields) => fields
            .iter()
            .map(|(field, message)| format!("{field} {message}"))
            .collect::<Vec<_>>()
            .join("; "),
        other => other.to_string(),
    }
}

fn prompt_password() -> anyhow::Result<String> {
    let password = rpassword::prompt_password("Password: ").context("reading password")?;
    let confirm = rpassword::prompt_password("Repeat password: ").context("reading password")?;
    if password != confirm {
        bail!("passwords do not match");
    }
    Ok(password)
}

fn read_password_from_stdin() -> anyhow::Result<String> {
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("reading password from stdin")?;
    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}
