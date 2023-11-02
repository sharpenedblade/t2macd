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
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy)]
enum FanCurve {
    LINEAR,
}

#[derive(Serialize, Deserialize)]
struct Config {
    fan_curve: FanCurve,
    min_temp: u32,
    max_temp: u32,
}

impl Config {
    fn get(path: &Path) -> Result<Config, std::io::Error> {
        match fs::read_to_string(path) {
            Ok(config_file) => match serde_json::from_str(&config_file) {
                Ok(config) => Ok(config),
                Err(..) => {
                    eprintln!("Could not parse config, using default config");
                    Ok(Config {
                        fan_curve: FanCurve::LINEAR,
                        min_temp: 80,
                        max_temp: 100,
                    })
                }
            },
            Err(error) => match error.kind() {
                io::ErrorKind::NotFound => {
                    let config = Config {
                        fan_curve: FanCurve::LINEAR,
                        min_temp: 80,
                        max_temp: 100,
                    };
                    match fs::write(path, serde_json::to_string(&config).unwrap()) {
                        Ok(..) => println!("Created default config"),
                        Err(..) => eprintln!("Failed to write default config"),
                    };
                    Ok(config)
                }
                _ => Err(error),
            },
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
    if all_fans.len() == 0 {
        panic!();
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

fn main() {
    let config = Config::get(&PathBuf::from("/etc/t2macd.json")).unwrap();
    let fans = match init_fans(&config) {
        Ok(fans) => fans,
        Err(..) => panic!("An error occured when initializing fans"),
    };
    loop {
        for fan in &fans {
            match fan.set_speed(fan.calc_speed(get_current_temp(), &config)) {
                Ok(..) => continue,
                Err(..) => println!("Error: Failed to set fan speed"),
            }
        }
    }
}
