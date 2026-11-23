//! Bench del motor VT sin ventana: throughput, resize y frame.
//!
//! Ejecutar en release para que los números sean comparables con los
//! presupuestos de ADR 0005:
//!
//! ```powershell
//! cargo run -p axf_terminal --release --example engine_bench
//! ```

#![expect(
    clippy::print_stdout,
    reason = "bench de diagnóstico: su salida es el entregable"
)]

use std::time::Instant;

use axf_terminal::EngineBench;

fn main() {
    throughput();
    resize();
    frame();
}

fn throughput() {
    let mut engine = EngineBench::new(80, 24, 8, 16);
    let chunk = b"\x1b[31mhola\x1b[0m\r\n".repeat(4096);
    let rounds = 64;
    let started = Instant::now();
    for _ in 0..rounds {
        engine.advance(&chunk);
    }
    let elapsed = started.elapsed();
    let bytes = chunk.len() * rounds;
    let mib = bytes as f64 / (1024.0 * 1024.0);
    println!(
        "throughput: {mib:.2} MiB en {:.3} s ({:.2} MiB/s)",
        elapsed.as_secs_f64(),
        mib / elapsed.as_secs_f64()
    );
}

fn resize() {
    let mut engine = EngineBench::new(200, 50, 8, 16);
    engine.advance(&vec![b'x'; 200 * 50]);
    let rounds = 100;
    let started = Instant::now();
    for index in 0..rounds {
        if index % 2 == 0 {
            engine.resize(100, 30);
        } else {
            engine.resize(200, 50);
        }
        let _ = engine.frame();
    }
    let elapsed = started.elapsed();
    println!(
        "resize: {rounds} ciclos 200x50<->100x30 en {:.3} s ({:.2} ms/ciclo)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e3 / rounds as f64
    );
}

fn frame() {
    let mut engine = EngineBench::new(200, 50, 8, 16);
    engine.advance(&vec![b'x'; 200 * 50]);
    let rounds = 100;
    let started = Instant::now();
    for _ in 0..rounds {
        let frame = engine.frame();
        std::hint::black_box(frame);
    }
    let elapsed = started.elapsed();
    println!(
        "frame 200x50: {rounds} frames en {:.3} s ({:.2} ms/frame)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e3 / rounds as f64
    );
}
