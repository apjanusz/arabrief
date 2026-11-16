//! Bounded workers start another job as soon as one finishes, without batch barriers.
use std::sync::Mutex;
pub(crate) fn run<T: Sync, R: Send>(
    items: &[T],
    limit: usize,
    work: impl Fn(&T) -> R + Sync,
    stop: impl Fn(&R) -> bool + Sync,
) -> Result<Vec<(usize, R)>, String> {
    assert!(limit > 0);
    let queue = Mutex::new((0usize, false));
    let mut results = std::thread::scope(|scope| {
        let mut jobs = Vec::new();
        for _ in 0..limit.min(items.len()) {
            let queue = &queue;
            let work = &work;
            let stop = &stop;
            jobs.push(scope.spawn(move || {
                let mut results = Vec::new();
                loop {
                    let index = {
                        let mut queue = queue.lock().unwrap_or_else(|e| e.into_inner());
                        if queue.1 || queue.0 >= items.len() {
                            break;
                        }
                        let index = queue.0;
                        queue.0 += 1;
                        index
                    };
                    let result = work(&items[index]);
                    if stop(&result) {
                        queue.lock().unwrap_or_else(|e| e.into_inner()).1 = true;
                    }
                    results.push((index, result));
                }
                results
            }));
        }
        let mut results = Vec::new();
        for job in jobs {
            results.extend(job.join().map_err(|_| "Błąd wątku pobierania wiadomości")?);
        }
        Ok::<_, String>(results)
    })?;
    results.sort_by_key(|(index, _)| *index);
    Ok(results)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{Duration, Instant},
    };
    #[test]
    fn bounds_workers_and_preserves_order() {
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let results = run(
            &(0..12).collect::<Vec<_>>(),
            3,
            |item| {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(3));
                active.fetch_sub(1, Ordering::SeqCst);
                item * 2
            },
            |_| false,
        )
        .unwrap();
        assert!(peak.load(Ordering::SeqCst) <= 3);
        assert_eq!(
            results.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            (0..12).map(|n| n * 2).collect::<Vec<_>>()
        );
        let stopped = run(&[1, 2, 3], 1, |item| *item, |_| true).unwrap();
        assert_eq!(stopped.len(), 1);
    }
    #[test]
    #[ignore = "explicit controlled performance comparison, no network or paid requests"]
    fn compare_batch_barrier_and_work_queue() {
        let data = [80u64, 5, 5, 5, 80, 5, 5, 5];
        let work = |ms: &u64| {
            std::thread::sleep(Duration::from_millis(*ms));
            *ms
        };
        let mut before = Vec::new();
        let mut after = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let mut old = Vec::new();
            for batch in data.chunks(4) {
                old.extend(std::thread::scope(|scope| {
                    let jobs: Vec<_> = batch
                        .iter()
                        .map(|ms| scope.spawn(move || work(ms)))
                        .collect();
                    jobs.into_iter()
                        .map(|job| job.join().unwrap())
                        .collect::<Vec<_>>()
                }));
            }
            before.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = Instant::now();
            let new = run(&data, 4, work, |_| false).unwrap();
            after.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(
                old,
                new.into_iter().map(|(_, value)| value).collect::<Vec<_>>()
            );
        }
        before.sort_by(f64::total_cmp);
        after.sort_by(f64::total_cmp);
        println!(
            "BENCH rss_queue jobs=8 workers=4 repetitions=5 before_median_ms={:.3} after_median_ms={:.3}",
            before[2], after[2]
        );
    }
}
