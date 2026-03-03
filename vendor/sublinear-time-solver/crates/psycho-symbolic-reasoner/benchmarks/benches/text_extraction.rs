use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use extractors::{TextExtractor, SentimentAnalyzer, PreferenceExtractor, EmotionDetector};
use fake::{Fake, faker::lorem::en::*};
use rand::prelude::*;

fn generate_test_texts(count: usize, word_range: std::ops::Range<usize>) -> Vec<String> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| {
            let word_count = rng.gen_range(word_range.clone());
            Words(word_count).fake::<Vec<String>>().join(" ")
        })
        .collect()
}

fn generate_emotional_text(count: usize) -> Vec<String> {
    let emotional_patterns = vec![
        "I absolutely love this amazing product! It makes me so happy and excited!",
        "This is terrible and disappointing. I hate how it never works properly.",
        "I'm feeling quite neutral about this. It's okay, nothing special really.",
        "What an incredible experience! I'm thrilled and overjoyed with the results!",
        "I'm frustrated and angry about this situation. It's completely unacceptable.",
        "This brings back wonderful memories of my childhood. So nostalgic and heartwarming.",
        "I'm worried and anxious about the future implications of this decision.",
        "Such a peaceful and calming environment. I feel so relaxed and content.",
    ];

    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| {
            let base = emotional_patterns.choose(&mut rng).unwrap();
            let additional_words: Vec<String> = Words(10..50).fake();
            format!("{} {}", base, additional_words.join(" "))
        })
        .collect()
}

fn generate_preference_text(count: usize) -> Vec<String> {
    let preference_patterns = vec![
        "I prefer coffee over tea, especially dark roast with no sugar.",
        "My favorite color is blue, but I also like green and purple shades.",
        "I enjoy reading science fiction novels and watching documentary films.",
        "I like to exercise in the morning, preferably running or cycling.",
        "Pizza is my favorite food, especially with pepperoni and mushrooms.",
        "I prefer working remotely rather than in a traditional office setting.",
        "Classical music helps me focus, though I enjoy jazz in the evenings.",
        "I like traveling to mountainous regions more than beach destinations.",
    ];

    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| {
            let base = preference_patterns.choose(&mut rng).unwrap();
            let additional_context: Vec<String> = Words(5..20).fake();
            format!("{} {}", base, additional_context.join(" "))
        })
        .collect()
}

fn bench_sentiment_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("sentiment_analysis");
    let analyzer = SentimentAnalyzer::new();

    // Test different text lengths
    for &word_count in [10, 50, 200, 1000].iter() {
        let texts = generate_test_texts(100, word_count..word_count + 10);

        group.throughput(Throughput::Elements(100));
        group.bench_with_input(
            BenchmarkId::new("words", word_count),
            &texts,
            |b, texts| {
                b.iter(|| {
                    for text in texts {
                        let result = analyzer.analyze(black_box(text));
                        black_box(result);
                    }
                });
            }
        );
    }

    // Test emotional content specifically
    let emotional_texts = generate_emotional_text(100);
    group.throughput(Throughput::Elements(100));
    group.bench_function("emotional_content", |b| {
        b.iter(|| {
            for text in &emotional_texts {
                let result = analyzer.analyze(black_box(text));
                black_box(result);
            }
        });
    });

    group.finish();
}

fn bench_preference_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("preference_extraction");
    let extractor = PreferenceExtractor::new();

    // Test different text complexities
    for &complexity in [50, 200, 500, 1000].iter() {
        let texts = generate_preference_text(50);

        group.throughput(Throughput::Elements(50));
        group.bench_with_input(
            BenchmarkId::new("complexity", complexity),
            &texts,
            |b, texts| {
                b.iter(|| {
                    for text in texts {
                        let result = extractor.extract(black_box(text));
                        black_box(result);
                    }
                });
            }
        );
    }

    group.finish();
}

fn bench_emotion_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("emotion_detection");
    let detector = EmotionDetector::new();

    // Test different emotional intensities
    let low_emotion_texts = generate_test_texts(100, 20..30);
    let high_emotion_texts = generate_emotional_text(100);

    group.throughput(Throughput::Elements(100));
    group.bench_function("low_emotion", |b| {
        b.iter(|| {
            for text in &low_emotion_texts {
                let result = detector.detect(black_box(text));
                black_box(result);
            }
        });
    });

    group.throughput(Throughput::Elements(100));
    group.bench_function("high_emotion", |b| {
        b.iter(|| {
            for text in &high_emotion_texts {
                let result = detector.detect(black_box(text));
                black_box(result);
            }
        });
    });

    group.finish();
}

fn bench_combined_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_analysis");
    let extractor = TextExtractor::new();

    for &text_count in [10, 50, 100, 500].iter() {
        let texts = generate_emotional_text(text_count);

        group.throughput(Throughput::Elements(text_count as u64));
        group.bench_with_input(
            BenchmarkId::new("full_analysis", text_count),
            &texts,
            |b, texts| {
                b.iter(|| {
                    for text in texts {
                        let result = extractor.analyze_all(black_box(text));
                        black_box(result);
                    }
                });
            }
        );
    }

    group.finish();
}

fn bench_regex_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_performance");

    // Test regex-heavy operations
    let texts_with_patterns = (0..100)
        .map(|_| {
            format!(
                "User {} prefers {} and likes {} but dislikes {}. Email: user{}@example.com Phone: +1-555-{:04}",
                fake::faker::name::en::Name().fake::<String>(),
                fake::faker::commerce::en::ProductName().fake::<String>(),
                fake::faker::commerce::en::ProductName().fake::<String>(),
                fake::faker::commerce::en::ProductName().fake::<String>(),
                rand::thread_rng().gen_range(1000..9999),
                rand::thread_rng().gen_range(1000..9999)
            )
        })
        .collect::<Vec<_>>();

    let extractor = TextExtractor::new();

    group.throughput(Throughput::Elements(100));
    group.bench_function("pattern_heavy_text", |b| {
        b.iter(|| {
            for text in &texts_with_patterns {
                let result = extractor.extract_preferences(black_box(text));
                black_box(result);
            }
        });
    });

    group.finish();
}

fn bench_unicode_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("unicode_handling");

    let unicode_texts = vec![
        "I love café au lait and crème brûlée! 😊 こんにちは世界",
        "Мне нравится борщ и водка! 🇷🇺 Très magnifique!",
        "我喜欢中文和日本料理 🍜 Español es hermoso también",
        "🎉🎊🎈 Emoji-heavy text with lots of symbols! 🚀🌟⭐",
        "عربي نص مع رموز تعبيرية 🕌 Mixed with English text",
    ];

    let repeated_unicode: Vec<String> = (0..50)
        .map(|i| format!("{} - Test iteration {}", unicode_texts[i % unicode_texts.len()], i))
        .collect();

    let extractor = TextExtractor::new();

    group.throughput(Throughput::Elements(50));
    group.bench_function("unicode_mixed", |b| {
        b.iter(|| {
            for text in &repeated_unicode {
                let result = extractor.analyze_all(black_box(text));
                black_box(result);
            }
        });
    });

    group.finish();
}

fn bench_memory_intensive_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_intensive");

    // Generate very large texts
    let large_texts: Vec<String> = (0..10)
        .map(|_| generate_test_texts(1, 5000..10000)[0].clone())
        .collect();

    let extractor = TextExtractor::new();

    group.throughput(Throughput::Elements(10));
    group.bench_function("large_text_processing", |b| {
        b.iter(|| {
            for text in &large_texts {
                let result = extractor.analyze_all(black_box(text));
                black_box(result);
            }
        });
    });

    group.finish();
}

fn bench_parallel_text_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_processing");

    let texts = generate_emotional_text(200);
    let chunk_size = 50;

    group.bench_function("sequential_processing", |b| {
        b.iter(|| {
            let extractor = TextExtractor::new();
            for text in &texts {
                let result = extractor.analyze_all(black_box(text));
                black_box(result);
            }
        });
    });

    group.bench_function("parallel_processing", |b| {
        b.iter(|| {
            let chunks: Vec<_> = texts.chunks(chunk_size).collect();
            let handles: Vec<_> = chunks.into_iter().map(|chunk| {
                let chunk = chunk.to_vec();
                std::thread::spawn(move || {
                    let extractor = TextExtractor::new();
                    let mut results = Vec::new();
                    for text in &chunk {
                        results.push(extractor.analyze_all(text));
                    }
                    results
                })
            }).collect();

            let _results: Vec<_> = handles.into_iter()
                .map(|h| h.join().unwrap())
                .collect();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sentiment_analysis,
    bench_preference_extraction,
    bench_emotion_detection,
    bench_combined_analysis,
    bench_regex_performance,
    bench_unicode_handling,
    bench_memory_intensive_operations,
    bench_parallel_text_processing
);

criterion_main!(benches);