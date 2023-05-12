// Copyright (C) 2023 t2macd contributors
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use glob::glob;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Serialize, Deserialize, Clone, Copy)]
enum FanCurve {
    LINEAR,
}

#[derive(Debug)]
enum ConfigError {
    IoError,
    ParseError,
    WriteError,
    NotFound,
}

#[derive(Serialize, Deserialize)]
struct Config {
    fan_curve: FanCurve,
    min_temp: u32,
    max_temp: u32,
}

impl Config {
    fn get(path: &Path) -> Result<Config, ConfigError> {
        match Config::read(path) {
            Ok(config) => Ok(config),
            Err(error) => match error {
                ConfigError::NotFound => {
                    let config = Config {
                        fan_curve: FanCurve::LINEAR,
                        min_temp: 80,
                        max_temp: 100,
                    };
                    match config.write(path) {
                        Ok(..) => (),
                        Err(..) => println!("Failed to write config"),
                    };
                    Ok(config)
                }
                ConfigError::IoError => Err(ConfigError::IoError),
                ConfigError::ParseError => {
                    println!("Config file corrupted. Is it well-formed?");
                    println!("Using default config");
                    Ok(Config {
                        fan_curve: FanCurve::LINEAR,
                        min_temp: 80,
                        max_temp: 100,
                    })
                }
                _ => {
                    panic!("This should never happen")
                }
            },
        }
    }

    fn write(&self, path: &Path) -> Result<(), ConfigError> {
        match fs::write(path, serde_json::to_string(self).unwrap()) {
            Ok(..) => Ok(()),
            Err(..) => Err(ConfigError::WriteError),
        }
    }

    fn read(path: &Path) -> Result<Config, ConfigError> {
        let raw: String = match fs::read_to_string(path) {
            Ok(string) => string,
            Err(error) => match error.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    println!(
                        "Permision Denied: Could not open {}",
                        path.to_str().unwrap()
                    );
                    return Err(ConfigError::IoError);
                }
                std::io::ErrorKind::NotFound => return Err(ConfigError::NotFound),
                _ => return Err(ConfigError::IoError),
            },
        };

        match serde_json::from_str(&raw) {
            Ok(config) => Ok(config),
            Err(..) => Err(ConfigError::ParseError),
        }
    }
}

struct Fan {
    path: PathBuf,
    max_speed: u32,
    min_speed: u32,
    speed_curve: FanCurve,
}

impl Fan {
    fn new(path: PathBuf, config: &Config) -> Result<Fan, std::io::Error> {
        let fan = Fan {
            max_speed: fs::read_to_string(Path::join(&path, "_max"))?
                .parse::<u32>()
                .unwrap(), // This file will always be an int
            min_speed: fs::read_to_string(Path::join(&path, "_min"))?
                .parse::<u32>()
                .unwrap(), // Same as above
            path,
            speed_curve: config.fan_curve.clone(),
        };
        fs::write(Path::join(&fan.path, "_manual"), "1")?;
        return Ok(fan);
    }

    fn set_speed(&self, speed: u32) -> Result<(), std::io::Error> {
        fs::write(Path::join(&self.path, "_output"), speed.to_string())
    }

    fn calc_speed(&self, current_temp: u32, config: &Config) -> u32 {
        match self.speed_curve {
            FanCurve::LINEAR => {
                (current_temp - config.min_temp) / (config.max_temp - config.min_temp)
                    * (self.max_speed - self.max_speed)
                    + self.min_speed
            }
        }
    }
}

fn init_fans(config: &Config) -> Result<Vec<Fan>, std::io::Error> {
    let mut all_fans = Vec::new();
    for i in glob("/sys/devices/*/*/*/*/APP0001:00/fan*_input").unwrap() {
        let mut i: String = String::from(i.unwrap().to_str().unwrap());
        i.truncate(i.len() - 6);
        let i: PathBuf = PathBuf::from(i);
        all_fans.push(Fan::new(i, config)?);
    }
    return Ok(all_fans);
}

fn get_current_temp() -> u32 {
    let mut cpu_temp_path: PathBuf = Default::default();
    for path in glob("/sys/devices/platform/coretemp.0/hwmon/hwmon*/temp1_input").unwrap() {
        cpu_temp_path = PathBuf::from(path.unwrap());
    }
    let cpu_temp: String = match fs::read_to_string(cpu_temp_path) {
        Ok(temp) => temp,
        Err(..) => panic!("Failed to read CPU temp. Are you running as root?"),
    };
    let cpu_temp: u32 = cpu_temp.parse::<u32>().unwrap(); // Always parsable

    let gpu_temp_path = PathBuf::from("/sys/class/drm/card0/device/hwmon/hwmon*/temp1_input");
    let gpu_temp: String = match fs::read_to_string(gpu_temp_path) {
        Ok(temp) => temp,
        Err(..) => panic!("Failed to read GPU temp. Are you running as root?"),
    };
    let gpu_temp: u32 = gpu_temp.parse::<u32>().unwrap(); // Same as above

    if gpu_temp > cpu_temp {
        gpu_temp
    } else {
        cpu_temp
    }
}

fn check_supported_env() -> bool {
    if env::consts::OS != "linux" {
        return false;
    }
    return true;
    // Checking for root requires unsafe code or extra dep
    // PLEASE DO NOT RUN AS A NORMAL USER
    // IT CAN BREAK
}

fn main() -> ExitCode {
    if !check_supported_env() {
        println!("YOU ARE NOT RUNNING THIS ON LINUX!!! THIS IS A LINUX TOOL");
        return ExitCode::from(2);
    }
    let config = Config::get(&PathBuf::from("/etc/t2macd.json")).unwrap();
    let fans = match init_fans(&config) {
        Ok(fans) => fans,
        Err(..) => panic!("An error occured when initializing fans"),
    };
    if fans.len() == 0 {
        println!("No fans found");
        return ExitCode::from(1);
    }
    loop {
        for fan in &fans {
            match fan.set_speed(fan.calc_speed(get_current_temp(), &config)) {
                Ok(..) => continue,
                Err(..) => println!("Error: Failed to set fan speed"),
            }
        }
    }
}
