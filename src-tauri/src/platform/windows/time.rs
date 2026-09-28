use crate::{
    error::{AppError, AppResult},
    pet::RandomSource,
    platform::contracts::{Clock, ClockSample},
};
use std::time::Duration;
use windows::Win32::{
    Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
    System::WindowsProgramming::QueryUnbiasedInterruptTime,
};

pub struct WindowsClock;
impl Clock for WindowsClock {
    fn sample(&self) -> AppResult<ClockSample> {
        let mut units = 0;
        // Windows 返回排除睡眠/休眠的 100ns 单位时间，不用墙上时间计算在线增量。
        if !unsafe { QueryUnbiasedInterruptTime(&mut units) }.as_bool() {
            return Err(AppError::new(
                "CLOCK_UNAVAILABLE",
                "读取系统运行时间失败，请重启应用",
            ));
        }
        Ok(ClockSample {
            utc: chrono::Utc::now(),
            running: Duration::from_millis(units / 10_000),
        })
    }
}

pub struct WindowsRandom;
impl RandomSource for WindowsRandom {
    fn below(&mut self, upper: u64) -> AppResult<u64> {
        if upper == 0 {
            return Err(AppError::storage());
        }
        let rejected = upper.wrapping_neg() % upper;
        loop {
            let mut bytes = [0; 8];
            unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
                .ok()
                .map_err(|_| AppError::new("RANDOM_SOURCE_ERROR", "读取系统随机数失败，请重试"))?;
            let value = u64::from_le_bytes(bytes);
            // 拒绝余数区域，避免直接取模导致分支概率偏斜。
            if value >= rejected {
                return Ok(value % upper);
            }
        }
    }
}
