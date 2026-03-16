// SiteOne Crawler - Initiator
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Parses CLI arguments, validates options, creates and returns Manager.

use crate::analysis::manager::AnalysisManager;
use crate::engine::manager::Manager;
use crate::error::CrawlerResult;
use crate::options::core_options;
use crate::utils;
use crate::version;

// Import all analyzers for registration
use crate::analysis::caching_analyzer::CachingAnalyzer;
use crate::analysis::content_type_analyzer::ContentTypeAnalyzer;
use crate::analysis::dns_analyzer::DnsAnalyzer;
use crate::analysis::external_links_analyzer::ExternalLinksAnalyzer;
use crate::analysis::fastest_analyzer::FastestAnalyzer;
use crate::analysis::headers_analyzer::HeadersAnalyzer;
use crate::analysis::page404_analyzer::Page404Analyzer;
use crate::analysis::redirects_analyzer::RedirectsAnalyzer;
use crate::analysis::skipped_urls_analyzer::SkippedUrlsAnalyzer;
use crate::analysis::slowest_analyzer::SlowestAnalyzer;
use crate::analysis::source_domains_analyzer::SourceDomainsAnalyzer;

// Import complex analyzers
use crate::analysis::accessibility_analyzer::AccessibilityAnalyzer;
use crate::analysis::best_practice_analyzer::BestPracticeAnalyzer;
use crate::analysis::security_analyzer::SecurityAnalyzer;
use crate::analysis::seo_opengraph_analyzer::SeoAndOpenGraphAnalyzer;
use crate::analysis::ssl_tls_analyzer::SslTlsAnalyzer;

pub struct Initiator {
    options: core_options::CoreOptions,
    analysis_manager: AnalysisManager,
}

impl Initiator {
    /// Create a new Initiator by parsing CLI arguments
    pub fn new(argv: &[String]) -> CrawlerResult<Self> {
        // Handle --help and --version before full parsing
        for arg in argv {
            if arg == "--help" || arg == "-h" {
                Self::print_help();
                std::process::exit(2);
            } else if arg == "--version" || arg == "-v" {
                println!(
                    "{}",
                    utils::get_color_text(&format!("Version: {}", version::CODE), "blue", false,)
                );
                std::process::exit(2);
            }
        }

        // Parse core options from argv
        let options = core_options::parse_argv(argv)?;

        // Handle special options that were parsed
        if options.show_help_only {
            Self::print_help();
            std::process::exit(2);
        }
        if options.show_version_only {
            println!(
                "{}",
                utils::get_color_text(&format!("Version: {}", version::CODE), "blue", false,)
            );
            std::process::exit(2);
        }

        // Create and populate analysis manager
        let mut analysis_manager = AnalysisManager::new();
        Self::register_analyzers(&mut analysis_manager, &options);
        analysis_manager.auto_activate_analyzers();

        // Apply analyzer filter regex if specified
        if let Some(ref filter_regex) = options.analyzer_filter_regex {
            analysis_manager.filter_analyzers_by_regex(filter_regex);
        }

        Ok(Self {
            options,
            analysis_manager,
        })
    }

    /// Create and return a Manager ready to run
    pub fn create_manager(self) -> CrawlerResult<Manager> {
        Manager::new(self.options, self.analysis_manager)
    }

    /// Get reference to parsed options
    pub fn get_options(&self) -> &core_options::CoreOptions {
        &self.options
    }

    /// Register all analyzers with the analysis manager.
    fn register_analyzers(analysis_manager: &mut AnalysisManager, options: &core_options::CoreOptions) {
        // Register all analyzers in alphabetical order
        analysis_manager.register_analyzer(Box::new(AccessibilityAnalyzer::new()));
        analysis_manager.register_analyzer(Box::new(BestPracticeAnalyzer::new()));
        analysis_manager.register_analyzer(Box::new(CachingAnalyzer::new()));
        analysis_manager.register_analyzer(Box::new(ContentTypeAnalyzer::new()));
        analysis_manager.register_analyzer(Box::new(DnsAnalyzer::new()));
        analysis_manager.register_analyzer(Box::new(ExternalLinksAnalyzer::new()));

        // FastestAnalyzer: pass fastest_top_limit and fastest_max_time from options
        let mut fastest = FastestAnalyzer::new();
        fastest.set_config(options.fastest_top_limit as usize, options.fastest_max_time);
        analysis_manager.register_analyzer(Box::new(fastest));

        analysis_manager.register_analyzer(Box::new(HeadersAnalyzer::new()));
        analysis_manager.register_analyzer(Box::new(Page404Analyzer::new()));
        analysis_manager.register_analyzer(Box::new(RedirectsAnalyzer::new()));
        analysis_manager.register_analyzer(Box::new(SecurityAnalyzer::new()));

        // SeoAndOpenGraphAnalyzer: pass max_heading_level from options
        let mut seo = SeoAndOpenGraphAnalyzer::new();
        seo.set_config(options.max_heading_level as i32);
        analysis_manager.register_analyzer(Box::new(seo));

        analysis_manager.register_analyzer(Box::new(SkippedUrlsAnalyzer::new()));

        // SlowestAnalyzer: pass slowest_top_limit, slowest_min_time, slowest_max_time from options
        let mut slowest = SlowestAnalyzer::new();
        slowest.set_config(
            options.slowest_top_limit as usize,
            options.slowest_min_time,
            options.slowest_max_time,
        );
        analysis_manager.register_analyzer(Box::new(slowest));

        analysis_manager.register_analyzer(Box::new(SourceDomainsAnalyzer::new()));
        analysis_manager.register_analyzer(Box::new(SslTlsAnalyzer::new()));
    }

    /// Print help text.
    pub fn print_help() {
        println!();
        println!(
            "{}",
            utils::get_color_text(
                "Usage: ./crawler --url=https://mydomain.tld/ [options]",
                "yellow",
                false,
            )
        );
        println!(
            "{}",
            utils::get_color_text(&format!("Version: {}", version::CODE), "blue", false,)
        );
        println!();

        let help_text = core_options::get_help_text();
        print!("{}", help_text);

        println!();
        println!("For more detailed descriptions of parameters, see README.md.");
        println!();
        println!(
            "{}{}{}",
            utils::get_color_text("Created with ", "gray", false),
            utils::get_color_text("\u{2665}", "red", false),
            utils::get_color_text(
                " by J\u{00e1}n Rege\u{0161} (jan.reges@siteone.cz) from www.SiteOne.io (Czech Republic) [2023-2026]",
                "gray",
                false,
            )
        );
    }
}
