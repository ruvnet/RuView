# WiFlow Reproduction Protocol

Status: Proposed research protocol

Tracks issue #1786.

## Objective

Determine whether a CSI optical flow style representation improves reusable camera free motion intelligence in RuView rather than only improving a proxy flow metric.

## Required capture metadata

Every run records hardware family, firmware, antenna count and geometry, channel width, center frequency, packet rate, timestamp source, synchronization method, room identity, participant identity class, label source, calibration, packet loss, missing subcarriers, preprocessing version, model commit, accelerator, software versions, power mode, and random seeds.

## Baselines

1. current RuView CSI preprocessing plus strongest task specific sensing head
2. WiFlow style motion field representation
3. simpler temporal derivative or phase and amplitude motion baseline

All conditions use identical train, validation, and held out environment splits.

## Metrics

Primary intermediate metrics:

* endpoint error or equivalent flow error
* directional consistency
* temporal stability
* uncertainty calibration

Required downstream metrics:

* tracking quality
* occupancy or presence quality
* pose or activity metric where available
* spatial state consistency in RuField or WorldGraph

Systems metrics:

* p50 and p95 inference latency
* peak memory
* CPU, GPU, or accelerator utilization
* energy per inference where measurable
* bytes per observation and sustained bandwidth

## Stress tests

Test unseen rooms, unseen participants, changed antenna geometry, a second RF hardware family, packet loss, burst loss, missing subcarriers, lower packet rate, clock jitter, and partial calibration failure.

## Promotion gate

The representation graduates only when the same learned representation improves at least two downstream RuView tasks on held out environments, or reaches equivalent downstream quality with materially lower edge cost.

A better optical flow proxy score alone is insufficient.

## Security and privacy

Visual or LiDAR labels may be used as privileged training evidence but must not become a runtime dependency for camera free deployment claims. Dataset provenance and consent requirements remain explicit.

## Rollback

The experiment is isolated behind a representation interface. Existing RuView sensing heads remain the production fallback.
