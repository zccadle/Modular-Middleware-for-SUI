use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc, TimeZone, NaiveDateTime, FixedOffset, Datelike}; // Added Datelike trait
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TimeConditionType {
    #[serde(rename = "at_time")]
    AtTime,
    #[serde(rename = "after_time")]
    AfterTime,
    #[serde(rename = "before_time")]
    BeforeTime,
    #[serde(rename = "between_times")]
    BetweenTimes,
    #[serde(rename = "on_weekday")]
    OnWeekday,
    #[serde(rename = "on_day")]
    OnDay,
    #[serde(rename = "in_month")]
    InMonth,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimeCondition {
    pub condition_type: TimeConditionType,
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub datetime: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub weekdays: Option<Vec<u8>>,
    #[serde(default)]
    pub days: Option<Vec<u8>>,
    #[serde(default)]
    pub months: Option<Vec<u8>>,
}

pub struct TimeBasedEvaluator;

impl TimeBasedEvaluator {
    pub fn evaluate(condition: &TimeCondition) -> Result<bool> {
        let now = Utc::now();
        
        match condition.condition_type {
            TimeConditionType::AtTime => {
                let target_time = Self::parse_time(condition)?;
                let diff = now.signed_duration_since(target_time).num_seconds().abs();
                // Allow a 5-second window for "at time" conditions
                Ok(diff <= 5)
            },
            
            TimeConditionType::AfterTime => {
                let target_time = Self::parse_time(condition)?;
                Ok(now > target_time)
            },
            
            TimeConditionType::BeforeTime => {
                let target_time = Self::parse_time(condition)?;
                Ok(now < target_time)
            },
            
            TimeConditionType::BetweenTimes => {
                let start_str = condition.start_time.as_deref()
                    .ok_or_else(|| anyhow!("Missing start_time for BetweenTimes condition"))?;
                let end_str = condition.end_time.as_deref()
                    .ok_or_else(|| anyhow!("Missing end_time for BetweenTimes condition"))?;
                
                
                let now_naive = now.naive_utc();
                let now_time = now_naive.time();
                
                let start_time = NaiveDateTime::parse_from_str(
                    &format!("{} {}", now_naive.date(), start_str),
                    "%Y-%m-%d %H:%M:%S"
                )?.time();
                
                let end_time = NaiveDateTime::parse_from_str(
                    &format!("{} {}", now_naive.date(), end_str),
                    "%Y-%m-%d %H:%M:%S"
                )?.time();
                
                if start_time < end_time {
                    Ok(now_time >= start_time && now_time <= end_time)
                } else {
                    // Handle overnight ranges (e.g., 22:00 - 06:00)
                    Ok(now_time >= start_time || now_time <= end_time)
                }
            },
            
            TimeConditionType::OnWeekday => {
                let weekdays = condition.weekdays.as_ref()
                    .ok_or_else(|| anyhow!("Missing weekdays for OnWeekday condition"))?;
                
                // Weekday where Monday is 1 and Sunday is 7
                let current_weekday = now.weekday().number_from_monday() as u8;
                Ok(weekdays.contains(&current_weekday))
            },
            
            TimeConditionType::OnDay => {
                let days = condition.days.as_ref()
                    .ok_or_else(|| anyhow!("Missing days for OnDay condition"))?;
                
                let current_day = now.day() as u8;
                Ok(days.contains(&current_day))
            },
            
            TimeConditionType::InMonth => {
                let months = condition.months.as_ref()
                    .ok_or_else(|| anyhow!("Missing months for InMonth condition"))?;
                
                let current_month = now.month() as u8;
                Ok(months.contains(&current_month))
            },
        }
    }
    
    fn parse_time(condition: &TimeCondition) -> Result<DateTime<Utc>> {
        if let Some(ts) = condition.timestamp {
            return Ok(Utc.timestamp_opt(ts, 0)
                .single()
                .ok_or_else(|| anyhow!("Invalid timestamp"))?);
        }
        
        if let Some(dt_str) = &condition.datetime {
            let tz_offset = match &condition.timezone {
                Some(tz) => Self::parse_timezone_offset(tz)?,
                None => 0, // Default to UTC
            };
            
            let fixed_offset = FixedOffset::east_opt(tz_offset)
                .ok_or_else(|| anyhow!("Invalid timezone offset"))?;
            
            let dt = DateTime::parse_from_str(dt_str, "%Y-%m-%d %H:%M:%S %z")
                .or_else(|_| {
                    // Try without timezone in string and use the provided offset
                    let naive = NaiveDateTime::parse_from_str(dt_str, "%Y-%m-%d %H:%M:%S")?;
                    // Fix: Explicitly specify the error type
                    let datetime = fixed_offset.from_local_datetime(&naive)
                        .single()
                        .ok_or_else(|| anyhow!("Invalid datetime"))?;
                    Ok::<DateTime<FixedOffset>, anyhow::Error>(datetime)
                })?;
            
            return Ok(dt.with_timezone(&Utc));
        }
        
        Err(anyhow!("No valid time specification provided"))
    }
    
    fn parse_timezone_offset(timezone: &str) -> Result<i32> {
        // Handle common timezone formats
        // E.g., "+0800", "+08:00", "UTC+8", "GMT-4"
        if timezone.starts_with('+') || timezone.starts_with('-') {
            let sign = if timezone.starts_with('+') { 1 } else { -1 };
            let tz_str = timezone[1..].replace(":", "");
            
            if tz_str.len() >= 3 {
                let hours = tz_str[0..2].parse::<i32>()?;
                let minutes = if tz_str.len() >= 4 {
                    tz_str[2..4].parse::<i32>()?
                } else {
                    0
                };
                
                return Ok(sign * (hours * 3600 + minutes * 60));
            }
        } else if timezone.starts_with("UTC") || timezone.starts_with("GMT") {
            let offset_part = &timezone[3..];
            if offset_part.starts_with('+') || offset_part.starts_with('-') {
                return Self::parse_timezone_offset(offset_part);
            } else if offset_part.is_empty() {
                return Ok(0); // UTC or GMT with no offset
            }
        }
        
        // Handle named timezones (simplified approach)
        match timezone {
            "EST" => Ok(-5 * 3600),
            "CST" => Ok(-6 * 3600),
            "MST" => Ok(-7 * 3600),
            "PST" => Ok(-8 * 3600),
            // Add more as needed
            _ => Err(anyhow!("Unsupported timezone format: {}", timezone)),
        }
    }
}