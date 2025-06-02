use crate::cache::Cache;
use crate::config::Config;
use crate::models::ProjectList;
use crate::scanner::ScanManager;
use anyhow::Result;

/// Get projects using cache if valid, otherwise scan fresh
pub fn get_projects_with_cache(config: &Config, verbose: bool) -> Result<ProjectList> {
    let cache = Cache::new(config)?;
    let _scan_manager = ScanManager::new();

    let cached_projects = cache.load_projects()?;
    let should_scan =
        cached_projects.is_none() || !cache.is_cache_valid(cache.projects_cache_path());

    if let Some(cached) = cached_projects {
        if !should_scan {
            if verbose {
                println!("Using cached projects");
            }
            return Ok(cached);
        } else if verbose {
            println!("Cache is stale, refreshing...");
        }
    } else if verbose {
        println!("Cache miss, scanning for projects...");
    }

    get_projects_fresh(config, verbose)
}

/// Get projects by scanning fresh (ignoring cache)
pub fn get_projects_fresh(config: &Config, verbose: bool) -> Result<ProjectList> {
    let cache = Cache::new(config)?;
    let scan_manager = ScanManager::new();

    let scan_start = std::time::Instant::now();
    let project_list = scan_manager.scan_all_verbose(config, verbose)?;
    let scan_duration = scan_start.elapsed();

    cache.save_projects(&project_list)?;

    if verbose {
        println!(
            "Found {} projects in {:.2?}",
            project_list.len(),
            scan_duration
        );
    }

    Ok(project_list)
}
