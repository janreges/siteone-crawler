// SiteOne Crawler - Main entry point
// (c) Jan Reges <jan.reges@siteone.cz>

use siteone_crawler::engine::initiator::Initiator;
use siteone_crawler::utils;

#[tokio::main]
async fn main() {
    // Install the default crypto provider for rustls (needed by SSL/TLS analyzer)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Force ANSI color output
    utils::force_enabled_colors();

    // Set timezone early, before tokio runtime spawns threads.
    // We check argv directly to avoid duplicating full option parsing.
    {
        let argv: Vec<String> = std::env::args().collect();
        for i in 0..argv.len() {
            if let Some(tz) = argv[i].strip_prefix("--timezone=") {
                // SAFETY: Called before any threads are spawned by the runtime
                unsafe {
                    std::env::set_var("TZ", tz);
                }
                break;
            } else if argv[i] == "--timezone" && i + 1 < argv.len() {
                unsafe {
                    std::env::set_var("TZ", &argv[i + 1]);
                }
                break;
            }
        }
    }

    let argv: Vec<String> = std::env::args().collect();

    // Create initiator (parses CLI args, handles --help/--version)
    // On error: show ERROR, then help, then ERROR again
    let initiator = match Initiator::new(&argv) {
        Ok(i) => i,
        Err(e) => {
            // Extract inner message (strip "Config error: " prefix for display)
            let msg = match &e {
                siteone_crawler::error::CrawlerError::Config(inner) => inner.clone(),
                other => other.to_string(),
            };
            eprint!("{}", utils::get_color_text(&format!("ERROR: {}", msg), "red", false));
            Initiator::print_help();
            eprintln!(
                "{}",
                utils::get_color_text(&format!("\nERROR: {}\n", msg), "red", false)
            );
            std::process::exit(101);
        }
    };

    // Check for serve mode (built-in HTTP server for browsing exports)
    let serve_markdown = initiator.get_options().serve_markdown_dir.clone();
    let serve_offline = initiator.get_options().serve_offline_dir.clone();
    let serve_port = initiator.get_options().serve_port as u16;
    let serve_bind = initiator.get_options().serve_bind_address.clone();

    if let Some(dir) = serve_markdown {
        siteone_crawler::server::run(
            std::path::PathBuf::from(dir),
            siteone_crawler::server::ServeMode::Markdown,
            serve_port,
            &serve_bind,
        )
        .await;
        return;
    }
    if let Some(dir) = serve_offline {
        siteone_crawler::server::run(
            std::path::PathBuf::from(dir),
            siteone_crawler::server::ServeMode::Offline,
            serve_port,
            &serve_bind,
        )
        .await;
        return;
    }

    // Create manager from initiator
    let mut manager = match initiator.create_manager() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error initializing crawler: {}", e);
            std::process::exit(1);
        }
    };

    // Run the crawler
    match manager.run().await {
        Ok(exit_code) => {
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
        Err(e) => {
            eprintln!("Crawler error: {}", e);
            std::process::exit(1);
        }
    }
}
