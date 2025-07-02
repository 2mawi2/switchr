use crate::cache::Cache;
use crate::config::Config;
use crate::models::ProjectList;
use crate::scanner::ScanManager;
use anyhow::Result;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

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

/// Get projects immediately from cache (even if stale) and optionally refresh in background
pub fn get_projects_with_background_refresh(
    config: &Config,
    verbose: bool,
) -> Result<(ProjectList, Option<Receiver<ProjectList>>)> {
    let cache = Cache::new(config)?;

    // Always load cached data first, even if stale
    let cached_projects = cache.load_projects()?.unwrap_or_else(ProjectList::new);

    // Check if we need to refresh
    let needs_refresh =
        cached_projects.is_empty() || !cache.is_cache_valid(cache.projects_cache_path());

    if needs_refresh {
        if verbose {
            println!("Starting background refresh...");
        }

        let (tx, rx) = channel();
        let config_clone = config.clone();

        // Spawn background thread to refresh
        thread::spawn(move || {
            if let Ok(fresh_projects) = get_projects_fresh(&config_clone, false) {
                // Ignore send errors (receiver might have been dropped)
                let _ = tx.send(fresh_projects);
            }
        });

        Ok((cached_projects, Some(rx)))
    } else {
        if verbose {
            println!("Using fresh cache");
        }
        Ok((cached_projects, None))
    }
}
