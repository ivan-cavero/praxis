//! Deploy commands — VPS deployment via SSH.
//!
//! NOTE: praxis is designed as a single-binary, local-first system.
//! VPS deployment is not part of the core vision. These commands exist
//! as placeholders for future remote-execution features and currently
//! print guidance rather than performing real operations.

/// Deploy configuration for VPS.
pub struct DeployConfig {
    pub host: String,
    pub user: String,
    pub port: u16,
    pub project_path: String,
}

impl DeployConfig {
    pub fn from_host(host: &str) -> Self {
        let parts: Vec<&str> = host.split('@').collect();
        let user = if parts.len() > 1 { parts[0] } else { "root" };
        let host = if parts.len() > 1 { parts[1] } else { parts[0] };

        Self {
            host: host.to_string(),
            user: user.to_string(),
            port: 22,
            project_path: "/opt/praxis".to_string(),
        }
    }
}

/// Setup VPS deployment.
pub async fn setup(host: &str) -> Result<(), String> {
    let config = DeployConfig::from_host(host);

    println!("  Host: {}@{}", config.user, config.host);
    println!("  Port: {}", config.port);
    println!("  Path: {}", config.project_path);
    println!();
    println!("  VPS deployment is not yet available.");
    println!("  praxis runs as a single binary locally — no server needed.");
    println!();
    println!("  To run praxis on a remote machine:");
    println!("    1. SSH into the remote machine");
    println!("    2. Install praxis: cargo install --path crates/cli");
    println!("    3. Run: praxis server  (starts the API + WebSocket)");
    println!("    4. Connect your dashboard to: http://<host>:8080");

    Ok(())
}

/// Push project to VPS.
pub async fn push() -> Result<(), String> {
    println!("  VPS push is not yet available.");
    println!("  To sync a project manually:");
    println!(
        "    rsync -avz ~/.config/praxis/projects/<name> user@host:~/.config/praxis/projects/"
    );

    Ok(())
}

/// Check VPS status.
pub async fn status() -> Result<(), String> {
    println!("  VPS status is not yet available.");
    println!("  To check a remote praxis instance:");
    println!("    curl http://<host>:8080/api/health");

    Ok(())
}

/// Stream logs from VPS.
pub async fn logs(tail: bool) -> Result<(), String> {
    if tail {
        println!("  VPS log streaming is not yet available.");
        println!("  To tail logs remotely:");
        println!("    ssh user@host 'tail -f ~/.config/praxis/praxis.log'");
    } else {
        println!("  VPS logs are not yet available.");
        println!("  To view logs remotely:");
        println!("    ssh user@host 'cat ~/.config/praxis/praxis.log'");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deploy_config_from_host() {
        let config = DeployConfig::from_host("user@myserver.com");
        assert_eq!(config.host, "myserver.com");
        assert_eq!(config.user, "user");
    }

    #[test]
    fn test_deploy_config_default_user() {
        let config = DeployConfig::from_host("myserver.com");
        assert_eq!(config.user, "root");
    }
}
