use std::ffi::OsStr;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Once;
use std::time::Duration;
use std::{env, fs};

use boxcars::ParserBuilder;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use replay_to_rocketsim::Converter;
use replay_to_rocketsim::rocketsim::init_from_default;

const DEFAULT_SAMPLE_SIZE: usize = 10;
const DEFAULT_CORPUS_LIMIT: usize = 100;

static ROCKETSIM_INIT: Once = Once::new();

const PLAYLISTS: [&str; 3] = ["1v1", "2v2", "3v3"];

#[derive(Debug)]
struct ReplayInput {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl ReplayInput {
    fn name(&self) -> String {
        self.path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("replay")
            .to_owned()
    }
}

#[derive(Debug)]
struct ReplaySet {
    playlist: &'static str,
    replays: Vec<ReplayInput>,
}

impl ReplaySet {
    fn total_bytes(&self) -> u64 {
        self.replays
            .iter()
            .map(|replay| replay.bytes.len() as u64)
            .sum()
    }
}

fn init_rocketsim() {
    ROCKETSIM_INIT.call_once(|| {
        init_from_default(true).expect("failed to initialize RocketSim collision meshes");
    });
}

fn playlist_replay_paths(playlist: &'static str, limit: usize) -> Vec<PathBuf> {
    let path = PathBuf::from("replays").join(playlist);

    fs::read_dir(&path)
        .unwrap_or_else(|err| panic!("failed to read replay directory {}: {err}", path.display()))
        .filter_map(|entry| {
            let entry = entry.unwrap_or_else(|err| {
                panic!("failed to read an entry from {}: {err}", path.display())
            });
            let path = entry.path();
            (path.extension() == Some(OsStr::new("replay"))).then_some(path)
        })
        .take(limit)
        .collect()
}

fn corpus_limit() -> usize {
    env::var("REPLAY_BENCH_LIMIT")
        .ok()
        .map_or(DEFAULT_CORPUS_LIMIT, |value| {
            value
                .parse::<usize>()
                .unwrap_or_else(|err| panic!("invalid REPLAY_BENCH_LIMIT={value:?}: {err}"))
        })
}

fn load_replay_sets() -> Vec<ReplaySet> {
    let limit = corpus_limit();
    assert!(limit > 0, "REPLAY_BENCH_LIMIT must be greater than 0");

    PLAYLISTS
        .into_iter()
        .map(|playlist| {
            let paths = playlist_replay_paths(playlist, limit);
            assert!(
                !paths.is_empty(),
                "replays/{playlist} did not contain any .replay files"
            );

            ReplaySet {
                playlist,
                replays: paths
                    .into_iter()
                    .map(|path| ReplayInput {
                        bytes: fs::read(&path).unwrap_or_else(|err| {
                            panic!("failed to read replay {}: {err}", path.display())
                        }),
                        path,
                    })
                    .collect(),
            }
        })
        .collect()
}

fn boxcars_parse_single_replay(c: &mut Criterion) {
    let replay_sets = load_replay_sets();
    let mut group = c.benchmark_group("boxcars_parse_single_replay");

    for replay_set in &replay_sets {
        let replay = &replay_set.replays[0];
        group.throughput(Throughput::Bytes(replay.bytes.len() as u64));
        group.bench_with_input(
            BenchmarkId::new(replay_set.playlist, replay.name()),
            &replay.bytes,
            |b, bytes| {
                b.iter(|| {
                    let replay = ParserBuilder::new(black_box(bytes))
                        .must_parse_network_data()
                        .parse()
                        .expect("failed to parse replay");
                    black_box(replay);
                });
            },
        );
    }

    group.finish();
}

fn boxcars_parse_replay_corpus(c: &mut Criterion) {
    let replay_sets = load_replay_sets();
    let mut group = c.benchmark_group("boxcars_parse_replay_corpus");

    for replay_set in &replay_sets {
        group.throughput(Throughput::Bytes(replay_set.total_bytes()));
        group.bench_with_input(
            BenchmarkId::new(
                replay_set.playlist,
                format!("{} replays", replay_set.replays.len()),
            ),
            &replay_set.replays,
            |b, replays| {
                b.iter(|| {
                    for replay in replays {
                        let parsed = ParserBuilder::new(black_box(&replay.bytes))
                            .must_parse_network_data()
                            .parse()
                            .unwrap_or_else(|err| {
                                panic!("failed to parse replay {}: {err}", replay.path.display())
                            });
                        black_box(parsed);
                    }
                });
            },
        );
    }

    group.finish();
}

fn convert_single_replay(c: &mut Criterion) {
    init_rocketsim();
    let replay_sets = load_replay_sets();
    let mut group = c.benchmark_group("convert_single_replay");

    for replay_set in &replay_sets {
        let replay = &replay_set.replays[0];
        group.throughput(Throughput::Bytes(replay.bytes.len() as u64));
        group.bench_with_input(
            BenchmarkId::new(replay_set.playlist, replay.name()),
            &replay.bytes,
            |b, bytes| {
                let converter = Converter::new();
                b.iter(|| {
                    let output = converter
                        .convert_bytes(black_box(bytes))
                        .expect("failed to convert replay");
                    black_box(output);
                });
            },
        );
    }

    group.finish();
}

fn convert_replay_corpus(c: &mut Criterion) {
    init_rocketsim();
    let replay_sets = load_replay_sets();
    let mut group = c.benchmark_group("convert_replay_corpus");

    for replay_set in &replay_sets {
        group.throughput(Throughput::Bytes(replay_set.total_bytes()));
        group.bench_with_input(
            BenchmarkId::new(
                replay_set.playlist,
                format!("{} replays", replay_set.replays.len()),
            ),
            &replay_set.replays,
            |b, replays| {
                b.iter(|| {
                    let converter = Converter::new();
                    for replay in replays {
                        let output = converter
                            .convert_bytes(black_box(&replay.bytes))
                            .unwrap_or_else(|err| {
                                panic!("failed to convert replay {}: {err}", replay.path.display())
                            });
                        black_box(output);
                    }
                });
            },
        );
    }

    group.finish();
}

fn criterion_config() -> Criterion {
    let sample_size =
        env::var("REPLAY_BENCH_SAMPLE_SIZE")
            .ok()
            .map_or(DEFAULT_SAMPLE_SIZE, |value| {
                value.parse::<usize>().unwrap_or_else(|err| {
                    panic!("invalid REPLAY_BENCH_SAMPLE_SIZE={value:?}: {err}")
                })
            });

    Criterion::default()
        .sample_size(sample_size)
        .measurement_time(Duration::from_secs(30))
        .warm_up_time(Duration::from_secs(5))
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets = boxcars_parse_single_replay,
        boxcars_parse_replay_corpus,
        convert_single_replay,
        convert_replay_corpus
}
criterion_main!(benches);
