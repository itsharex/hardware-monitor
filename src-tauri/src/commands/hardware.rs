use crate::{log_debug, log_error, log_internal, log_warn};
use nvapi;
use nvapi::UtilizationDomain;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use sysinfo::System;
use tauri::command;

pub struct AppState {
  pub system: Arc<Mutex<System>>,
  pub cpu_history: Arc<Mutex<VecDeque<f32>>>,
  pub memory_history: Arc<Mutex<VecDeque<f32>>>,
  pub gpu_history: Arc<Mutex<VecDeque<f32>>>,
  pub gpu_usage: Arc<Mutex<f32>>,
}

///
/// システム情報の更新頻度（秒）
///
const SYSTEM_INFO_INIT_INTERVAL: u64 = 1;

///
/// データを保持する期間（秒）
///
const HISTORY_CAPACITY: usize = 60;

///
/// ## CPU使用率（%）を取得
///
/// - pram state: `tauri::State<AppState>` アプリケーションの状態
/// - return: `i32` CPU使用率（%）
///
#[command]
pub fn get_cpu_usage(state: tauri::State<'_, AppState>) -> i32 {
  let system = state.system.lock().unwrap();
  let cpus = system.cpus();
  let total_usage: f32 = cpus.iter().map(|cpu| cpu.cpu_usage()).sum();

  let usage = total_usage / cpus.len() as f32;
  usage.round() as i32
}

///
/// ## メモリ使用率（%）を取得
///
/// - pram state: `tauri::State<AppState>` アプリケーションの状態
/// - return: `i32` メモリ使用率（%）
///
#[command]
pub fn get_memory_usage(state: tauri::State<'_, AppState>) -> i32 {
  let system = state.system.lock().unwrap();
  let used_memory = system.used_memory() as f64;
  let total_memory = system.total_memory() as f64;

  ((used_memory / total_memory) * 100.0 as f64).round() as i32
}

///
/// ## GPU使用率（%）を取得（Nvidia 限定）
///
/// - param state: `tauri::State<AppState>` アプリケーションの状態
/// - return: `i32` GPU使用率（%）
///
#[command]
pub fn get_gpu_usage(state: tauri::State<'_, AppState>) -> i32 {
  let gpu_usage = state.gpu_usage.lock().unwrap();
  *gpu_usage as i32
}

///
/// ## CPU使用率の履歴を取得
///
/// - param state: `tauri::State<AppState>` アプリケーションの状態
/// - param seconds: `usize` 取得する秒数
///
#[command]
pub fn get_cpu_usage_history(
  state: tauri::State<'_, AppState>,
  seconds: usize,
) -> Vec<f32> {
  let history = state.cpu_history.lock().unwrap();
  history.iter().rev().take(seconds).cloned().collect()
}

///
/// ## メモリ使用率の履歴を取得
///
/// - param state: `tauri::State<AppState>` アプリケーションの状態
/// - param seconds: `usize` 取得する秒数
///
#[command]
pub fn get_memory_usage_history(
  state: tauri::State<'_, AppState>,
  seconds: usize,
) -> Vec<f32> {
  let history = state.memory_history.lock().unwrap();
  history.iter().rev().take(seconds).cloned().collect()
}

///
/// ## GPU使用率の履歴を取得
///
/// - param state: `tauri::State<AppState>` アプリケーションの状態
/// - param seconds: `usize` 取得する秒数
///
#[command]
pub fn get_gpu_usage_history(
  state: tauri::State<'_, AppState>,
  seconds: usize,
) -> Vec<f32> {
  let history = state.gpu_history.lock().unwrap();
  history.iter().rev().take(seconds).cloned().collect()
}

///
/// ## システム情報の初期化
///
/// - param system: `Arc<Mutex<System>>` システム情報
///
/// - `SYSTEM_INFO_INIT_INTERVAL` 秒ごとにCPU使用率とメモリ使用率を更新
///
pub fn initialize_system(
  system: Arc<Mutex<System>>,
  cpu_history: Arc<Mutex<VecDeque<f32>>>,
  memory_history: Arc<Mutex<VecDeque<f32>>>,
  gpu_usage: Arc<Mutex<f32>>,
  gpu_history: Arc<Mutex<VecDeque<f32>>>,
) {
  thread::spawn(move || loop {
    {
      let mut sys = match system.lock() {
        Ok(s) => s,
        Err(_) => continue, // エラーハンドリング：ロックが破損している場合はスキップ
      };

      sys.refresh_cpu_all();
      sys.refresh_memory();

      let cpu_usage = {
        let cpus = sys.cpus();
        let total_usage: f32 = cpus.iter().map(|cpu| cpu.cpu_usage()).sum();
        (total_usage / cpus.len() as f32).round() as f32
      };

      let memory_usage = {
        let used_memory = sys.used_memory() as f64;
        let total_memory = sys.total_memory() as f64;
        (used_memory / total_memory * 100.0).round() as f32
      };

      //let gpu_usage_value = match get_gpu_usage() {
      //  Ok(usage) => usage,
      //  Err(_) => 0.0, // エラーが発生した場合はデフォルト値として0.0を使用
      //};

      //{
      //  let mut gpu = gpu_usage.lock().unwrap();
      //  *gpu = gpu_usage_value;
      //}

      {
        let mut cpu_hist = cpu_history.lock().unwrap();
        if cpu_hist.len() >= HISTORY_CAPACITY {
          cpu_hist.pop_front();
        }
        cpu_hist.push_back(cpu_usage);
      }

      {
        let mut memory_hist = memory_history.lock().unwrap();
        if memory_hist.len() >= HISTORY_CAPACITY {
          memory_hist.pop_front();
        }
        memory_hist.push_back(memory_usage);
      }

      //{
      //  let mut gpu_hist = gpu_history.lock().unwrap();
      //  if gpu_hist.len() >= HISTORY_CAPACITY {
      //    gpu_hist.pop_front();
      //  }
      //  gpu_hist.push_back(gpu_usage_value);
      //}
    }

    thread::sleep(Duration::from_secs(SYSTEM_INFO_INIT_INTERVAL));
  });

  ///
  /// TODO GPU使用率を取得する
  ///
  #[allow(dead_code)]
  fn get_gpu_usage() -> Result<f32, nvapi::Status> {
    log_debug!("start", "get_gpu_usage", None::<&str>);

    let gpus = nvapi::PhysicalGpu::enumerate()?;

    print!("{:?}", gpus);

    if gpus.is_empty() {
      log_warn!("not found", "get_gpu_usage", Some("gpu is not found"));
      tracing::warn!("gpu is not found");
      return Err(nvapi::Status::Error); // GPUが見つからない場合はエラーを返す
    }

    let mut total_usage = 0.0;
    let mut gpu_count = 0;

    for gpu in gpus.iter() {
      let usage = match gpu.usages() {
        Ok(usage) => usage,
        Err(e) => {
          log_error!("usages_failed", "get_gpu_usage", Some(e.to_string()));
          return Err(e);
        }
      };

      if let Some(gpu_usage) = usage.get(&UtilizationDomain::Graphics) {
        let usage_f32 = gpu_usage.0 as f32 / 100.0; // Percentage を f32 に変換
        total_usage += usage_f32;
        gpu_count += 1;
      }
    }

    if gpu_count == 0 {
      log_warn!(
        "no_usage",
        "get_gpu_usage",
        Some("No GPU usage data collected")
      );
      return Err(nvapi::Status::Error); // 使用率が取得できなかった場合のエラーハンドリング
    }

    let average_usage = total_usage / gpu_count as f32;

    log_debug!("end", "get_gpu_usage", None::<&str>);

    Ok(average_usage)
  }
}
