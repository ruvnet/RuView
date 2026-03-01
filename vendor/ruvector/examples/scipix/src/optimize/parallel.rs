//! Parallel processing utilities for OCR pipeline
//!
//! Provides parallel image preprocessing, batch OCR, and pipelined execution.

use image::DynamicImage;
use rayon::prelude::*;
use std::sync::Arc;
use tokio::sync::Semaphore;

use super::parallel_enabled;

/// Parallel preprocessing of multiple images
pub fn parallel_preprocess<F>(images: Vec<DynamicImage>, preprocess_fn: F) -> Vec<DynamicImage>
where
    F: Fn(DynamicImage) -> DynamicImage + Sync + Send,
{
    if !parallel_enabled() {
        return images.into_iter().map(preprocess_fn).collect();
    }

    images.into_par_iter().map(preprocess_fn).collect()
}

/// Parallel processing with error handling
pub fn parallel_preprocess_result<F, E>(
    images: Vec<DynamicImage>,
    preprocess_fn: F,
) -> Vec<std::result::Result<DynamicImage, E>>
where
    F: Fn(DynamicImage) -> std::result::Result<DynamicImage, E> + Sync + Send,
    E: Send,
{
    if !parallel_enabled() {
        return images.into_iter().map(preprocess_fn).collect();
    }

    images.into_par_iter().map(preprocess_fn).collect()
}

/// Pipeline parallel execution for OCR workflow
///
/// Executes stages in a pipeline: preprocess | detect | recognize
/// Each stage can start processing the next item while previous stages
/// continue with subsequent items.
pub struct PipelineExecutor<T, U, V> {
    stage1: Arc<dyn Fn(T) -> U + Send + Sync>,
    stage2: Arc<dyn Fn(U) -> V + Send + Sync>,
}

impl<T, U, V> PipelineExecutor<T, U, V>
where
    T: Send,
    U: Send,
    V: Send,
{
    pub fn new<F1, F2>(stage1: F1, stage2: F2) -> Self
    where
        F1: Fn(T) -> U + Send + Sync + 'static,
        F2: Fn(U) -> V + Send + Sync + 'static,
    {
        Self {
            stage1: Arc::new(stage1),
            stage2: Arc::new(stage2),
        }
    }

    /// Execute pipeline on multiple inputs
    pub fn execute_batch(&self, inputs: Vec<T>) -> Vec<V> {
        if !parallel_enabled() {
            return inputs
                .into_iter()
                .map(|input| {
                    let stage1_out = (self.stage1)(input);
                    (self.stage2)(stage1_out)
                })
                .collect();
        }

        inputs
            .into_par_iter()
            .map(|input| {
                let stage1_out = (self.stage1)(input);
                (self.stage2)(stage1_out)
            })
            .collect()
    }
}

/// Three-stage pipeline executor
pub struct Pipeline3<T, U, V, W> {
    stage1: Arc<dyn Fn(T) -> U + Send + Sync>,
    stage2: Arc<dyn Fn(U) -> V + Send + Sync>,
    stage3: Arc<dyn Fn(V) -> W + Send + Sync>,
}

impl<T, U, V, W> Pipeline3<T, U, V, W>
where
    T: Send,
    U: Send,
    V: Send,
    W: Send,
{
    pub fn new<F1, F2, F3>(stage1: F1, stage2: F2, stage3: F3) -> Self
    where
        F1: Fn(T) -> U + Send + Sync + 'static,
        F2: Fn(U) -> V + Send + Sync + 'static,
        F3: Fn(V) -> W + Send + Sync + 'static,
    {
        Self {
            stage1: Arc::new(stage1),
            stage2: Arc::new(stage2),
            stage3: Arc::new(stage3),
        }
    }

    pub fn execute_batch(&self, inputs: Vec<T>) -> Vec<W> {
        if !parallel_enabled() {
            return inputs
                .into_iter()
                .map(|input| {
                    let out1 = (self.stage1)(input);
                    let out2 = (self.stage2)(out1);
                    (self.stage3)(out2)
                })
                .collect();
        }

        inputs
            .into_par_iter()
            .map(|input| {
                let out1 = (self.stage1)(input);
                let out2 = (self.stage2)(out1);
                (self.stage3)(out2)
            })
            .collect()
    }
}

/// Parallel map with configurable chunk size
pub fn parallel_map_chunked<T, U, F>(items: Vec<T>, chunk_size: usize, map_fn: F) -> Vec<U>
where
    T: Send,
    U: Send,
    F: Fn(T) -> U + Sync + Send,
{
    if !parallel_enabled() {
        return items.into_iter().map(map_fn).collect();
    }

    items
        .into_par_iter()
        .with_min_len(chunk_size)
        .map(map_fn)
        .collect()
}

/// Async parallel executor with concurrency limit
pub struct AsyncParallelExecutor {
    semaphore: Arc<Semaphore>,
}

impl AsyncParallelExecutor {
    /// Create executor with maximum concurrency limit
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Execute async tasks with concurrency limit
    pub async fn execute<T, F, Fut>(&self, tasks: Vec<T>, executor: F) -> Vec<Fut::Output>
    where
        T: Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        let mut handles = Vec::new();

        for task in tasks {
            let permit = self.semaphore.clone().acquire_owned().await.unwrap();
            let executor = executor.clone();

            let handle = tokio::spawn(async move {
                let result = executor(task).await;
                drop(permit); // Release semaphore
                result
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        let mut results = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }

        results
    }

    /// Execute with error handling
    pub async fn execute_result<T, F, Fut, R, E>(
        &self,
        tasks: Vec<T>,
        executor: F,
    ) -> Vec<std::result::Result<R, E>>
    where
        T: Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = std::result::Result<R, E>> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let mut handles = Vec::new();

        for task in tasks {
            let permit = self.semaphore.clone().acquire_owned().await.unwrap();
            let executor = executor.clone();

            let handle = tokio::spawn(async move {
                let result = executor(task).await;
                drop(permit);
                result
            });

            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(_) => continue, // Task panicked
            }
        }

        results
    }
}

/// Work-stealing parallel iterator for unbalanced workloads
pub fn parallel_unbalanced<T, U, F>(items: Vec<T>, map_fn: F) -> Vec<U>
where
    T: Send,
    U: Send,
    F: Fn(T) -> U + Sync + Send,
{
    if !parallel_enabled() {
        return items.into_iter().map(map_fn).collect();
    }

    // Use adaptive strategy for unbalanced work
    items
        .into_par_iter()
        .with_min_len(1) // Allow fine-grained work stealing
        .map(map_fn)
        .collect()
}

/// Get optimal thread count for current system
pub fn optimal_thread_count() -> usize {
    rayon::current_num_threads()
}

/// Set global thread pool size
pub fn set_thread_count(threads: usize) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_map() {
        let data: Vec<i32> = (0..100).collect();
        let result = parallel_map_chunked(data, 10, |x| x * 2);

        assert_eq!(result.len(), 100);
        assert_eq!(result[0], 0);
        assert_eq!(result[50], 100);
        assert_eq!(result[99], 198);
    }

    #[test]
    fn test_pipeline_executor() {
        let pipeline = PipelineExecutor::new(|x: i32| x + 1, |x: i32| x * 2);

        let inputs = vec![1, 2, 3, 4, 5];
        let results = pipeline.execute_batch(inputs);

        assert_eq!(results, vec![4, 6, 8, 10, 12]);
    }

    #[test]
    fn test_pipeline3() {
        let pipeline = Pipeline3::new(|x: i32| x + 1, |x: i32| x * 2, |x: i32| x - 1);

        let inputs = vec![1, 2, 3];
        let results = pipeline.execute_batch(inputs);

        // (1+1)*2-1 = 3, (2+1)*2-1 = 5, (3+1)*2-1 = 7
        assert_eq!(results, vec![3, 5, 7]);
    }

    #[tokio::test]
    async fn test_async_executor() {
        let executor = AsyncParallelExecutor::new(2);

        let tasks = vec![1, 2, 3, 4, 5];
        let results = executor
            .execute(tasks, |x| async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                x * 2
            })
            .await;

        assert_eq!(results.len(), 5);
        assert!(results.contains(&2));
        assert!(results.contains(&10));
    }

    #[test]
    fn test_optimal_threads() {
        let threads = optimal_thread_count();
        assert!(threads > 0);
        assert!(threads <= num_cpus::get());
    }
}
