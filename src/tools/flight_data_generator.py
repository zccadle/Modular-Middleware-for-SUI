#!/usr/bin/env python3
"""
Flight Data Generator

This script generates realistic mock flight data for testing the flight insurance application.
It produces delay patterns that mimic real-world scenarios, including different delay
distributions based on airline, time of day, and weather conditions.
"""

import json
import random
import argparse
import os
import datetime
import math
from typing import Dict, List, Any, Tuple, Optional
import csv
import matplotlib.pyplot as plt
import numpy as np

# Constants for delay generation
AIRLINES = [
    {"code": "BA", "name": "British Airways", "delay_factor": 0.8},  
    {"code": "LH", "name": "Lufthansa", "delay_factor": 0.7},
    {"code": "AF", "name": "Air France", "delay_factor": 0.85},
    {"code": "DL", "name": "Delta", "delay_factor": 0.75},
    {"code": "UA", "name": "United Airlines", "delay_factor": 0.9},
    {"code": "AA", "name": "American Airlines", "delay_factor": 0.83},
    {"code": "EK", "name": "Emirates", "delay_factor": 0.65},
    {"code": "TK", "name": "Turkish Airlines", "delay_factor": 0.88},
    {"code": "LX", "name": "Swiss", "delay_factor": 0.6},
    {"code": "FR", "name": "Ryanair", "delay_factor": 1.1}
]

AIRPORTS = [
    {"code": "LHR", "name": "London Heathrow", "congestion_factor": 1.3},
    {"code": "CDG", "name": "Paris Charles de Gaulle", "congestion_factor": 1.2},
    {"code": "FRA", "name": "Frankfurt", "congestion_factor": 1.1},
    {"code": "JFK", "name": "New York JFK", "congestion_factor": 1.4},
    {"code": "LAX", "name": "Los Angeles", "congestion_factor": 1.25},
    {"code": "DXB", "name": "Dubai", "congestion_factor": 0.9},
    {"code": "SIN", "name": "Singapore Changi", "congestion_factor": 0.8},
    {"code": "IST", "name": "Istanbul", "congestion_factor": 1.15},
    {"code": "AMS", "name": "Amsterdam Schiphol", "congestion_factor": 1.05},
    {"code": "MAD", "name": "Madrid Barajas", "congestion_factor": 1.1}
]

WEATHER_CONDITIONS = [
    {"condition": "Clear", "probability": 0.6, "delay_factor": 0.7},
    {"condition": "Cloudy", "probability": 0.15, "delay_factor": 0.9},
    {"condition": "Rain", "probability": 0.1, "delay_factor": 1.5},
    {"condition": "Snow", "probability": 0.05, "delay_factor": 2.5},
    {"condition": "Fog", "probability": 0.05, "delay_factor": 2.0},
    {"condition": "Thunderstorm", "probability": 0.03, "delay_factor": 3.0},
    {"condition": "High Winds", "probability": 0.02, "delay_factor": 1.8}
]

# Time of day factors (24-hour format)
TIME_FACTORS = {
    # Early morning (less congestion)
    5: 0.7, 6: 0.75, 7: 0.9, 
    # Morning rush
    8: 1.3, 9: 1.4, 10: 1.2,
    # Midday
    11: 1.0, 12: 1.0, 13: 1.0, 14: 1.0, 
    # Afternoon rush
    15: 1.1, 16: 1.3, 17: 1.5, 18: 1.4,
    # Evening
    19: 1.2, 20: 1.1, 21: 1.0, 22: 0.9,
    # Late night
    23: 0.8, 0: 0.7, 1: 0.6, 2: 0.6, 3: 0.6, 4: 0.6
}

# Day of week factors (0 = Monday, 6 = Sunday)
DAY_FACTORS = {
    0: 1.0,  # Monday
    1: 0.9,  # Tuesday
    2: 0.9,  # Wednesday
    3: 1.0,  # Thursday
    4: 1.3,  # Friday
    5: 1.2,  # Saturday
    6: 1.1   # Sunday
}

def generate_flight_number(airline_code: str) -> str:
    """Generate a realistic flight number for an airline."""
    return f"{airline_code}{random.randint(100, 9999)}"

def sample_from_weighted_list(items: List[Dict[str, Any]], weight_key: str) -> Dict[str, Any]:
    """Sample an item from a list based on weights."""
    weights = [item.get(weight_key, 1.0) for item in items]
    total_weight = sum(weights)
    weights = [w / total_weight for w in weights]
    return random.choices(items, weights=weights, k=1)[0]

def generate_delay_minutes(
    airline_factor: float, 
    origin_factor: float, 
    destination_factor: float,
    weather_factor: float,
    time_factor: float,
    day_factor: float,
    cancelled: bool
) -> int:
    """Generate realistic delay minutes based on various factors."""
    if cancelled:
        return 0  # No delay time for cancelled flights
    
    # Base delay formula with exponential component to create realistic distribution
    base_mean = 15.0  # Base mean delay in minutes
    combined_factor = (
        airline_factor * 
        math.sqrt(origin_factor * destination_factor) * 
        weather_factor * 
        time_factor * 
        day_factor
    )
    
    # Use exponential distribution for delay
    mean_delay = base_mean * combined_factor
    
    # Random component - exponential distribution creates more realistic delays
    # with many short delays and fewer long ones
    if random.random() < 0.7:  # 70% chance of small delay
        delay = random.expovariate(1.0 / (mean_delay / 2))
    else:  # 30% chance of larger delay
        delay = random.expovariate(1.0 / (mean_delay * 2))
    
    return max(0, int(round(delay)))

def get_flight_status(delay_minutes: int, cancelled: bool) -> str:
    """Determine flight status based on delay and cancellation."""
    if cancelled:
        return "cancelled"
    elif delay_minutes >= 180:  # 3+ hours
        return "significantly_delayed"
    elif delay_minutes >= 45:   # 45+ minutes
        return "delayed"
    elif delay_minutes >= 15:   # 15+ minutes
        return "slightly_delayed"
    else:
        return "on_time"

def determine_cancellation(
    weather_condition: Dict[str, Any],
    airline_factor: float
) -> bool:
    """Determine if a flight is cancelled based on weather and airline reliability."""
    # Base cancellation probability
    base_probability = 0.01  # 1% base cancellation rate
    
    # Weather factor - higher for severe weather
    weather_multiplier = {
        "Clear": 0.2,
        "Cloudy": 0.3,
        "Rain": 1.5,
        "Snow": 10.0,
        "Fog": 6.0,
        "Thunderstorm": 15.0,
        "High Winds": 8.0
    }
    
    weather_factor = weather_multiplier.get(weather_condition["condition"], 1.0)
    
    # Final cancellation probability
    cancellation_probability = base_probability * weather_factor * airline_factor
    
    # Cap at reasonable value
    cancellation_probability = min(cancellation_probability, 0.5)
    
    return random.random() < cancellation_probability

def generate_flight_data(
    flight_date: datetime.datetime,
    num_flights: int = 100
) -> List[Dict[str, Any]]:
    """Generate multiple flight records for a given date."""
    flights = []
    
    for _ in range(num_flights):
        # Select airline, origin, and destination
        airline = random.choice(AIRLINES)
        origin = random.choice(AIRPORTS)
        
        # Ensure destination is different from origin
        possible_destinations = [a for a in AIRPORTS if a["code"] != origin["code"]]
        destination = random.choice(possible_destinations)
        
        # Random departure time within the day
        departure_hour = random.randint(0, 23)
        departure_minute = random.choice([0, 15, 30, 45])
        
        # Create departure time
        departure_time = flight_date.replace(
            hour=departure_hour,
            minute=departure_minute,
            second=0,
            microsecond=0
        )
        
        # Flight duration - based on airport distance (simplified for demo)
        base_duration_minutes = random.randint(60, 720)  # 1 to 12 hours
        scheduled_arrival = departure_time + datetime.timedelta(minutes=base_duration_minutes)
        
        # Weather at origin
        origin_weather = sample_from_weighted_list(WEATHER_CONDITIONS, "probability")
        
        # Determine if flight is cancelled
        cancelled = determine_cancellation(origin_weather, airline["delay_factor"])
        
        # Calculate delay based on various factors
        time_factor = TIME_FACTORS.get(departure_hour, 1.0)
        day_factor = DAY_FACTORS.get(departure_time.weekday(), 1.0)
        
        delay_minutes = generate_delay_minutes(
            airline_factor=airline["delay_factor"],
            origin_factor=origin["congestion_factor"],
            destination_factor=destination["congestion_factor"],
            weather_factor=origin_weather["delay_factor"],
            time_factor=time_factor,
            day_factor=day_factor,
            cancelled=cancelled
        )
        
        # Calculate actual times based on delay
        estimated_departure = departure_time + datetime.timedelta(minutes=delay_minutes)
        estimated_arrival = scheduled_arrival + datetime.timedelta(minutes=delay_minutes)
        
        # Status
        status = get_flight_status(delay_minutes, cancelled)
        
        # Flight number
        flight_number = generate_flight_number(airline["code"])
        
        # Create flight record
        flight = {
            "flight_number": flight_number,
            "airline_code": airline["code"],
            "airline_name": airline["name"],
            "origin_code": origin["code"],
            "origin_name": origin["name"],
            "destination_code": destination["code"],
            "destination_name": destination["name"],
            "scheduled_departure": departure_time.isoformat(),
            "estimated_departure": estimated_departure.isoformat() if not cancelled else None,
            "actual_departure": None,  # Would be populated after flight departs
            "scheduled_arrival": scheduled_arrival.isoformat(),
            "estimated_arrival": estimated_arrival.isoformat() if not cancelled else None,
            "actual_arrival": None,  # Would be populated after flight arrives
            "status": status,
            "delay_minutes": delay_minutes,
            "cancelled": cancelled,
            "weather_condition": origin_weather["condition"]
        }
        
        flights.append(flight)
    
    return flights

def generate_dataset(start_date: datetime.datetime, days: int, flights_per_day: int) -> List[Dict[str, Any]]:
    """Generate a dataset spanning multiple days."""
    all_flights = []
    
    for day in range(days):
        current_date = start_date + datetime.timedelta(days=day)
        daily_flights = generate_flight_data(current_date, flights_per_day)
        all_flights.extend(daily_flights)
    
    return all_flights

def save_json(flights: List[Dict[str, Any]], filename: str):
    """Save flight data as JSON."""
    with open(filename, 'w') as f:
        json.dump(flights, f, indent=2)
    print(f"Saved {len(flights)} flights to {filename}")

def save_csv(flights: List[Dict[str, Any]], filename: str):
    """Save flight data as CSV."""
    if not flights:
        print("No flights to save")
        return
    
    fieldnames = flights[0].keys()
    
    with open(filename, 'w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(flights)
    
    print(f"Saved {len(flights)} flights to {filename}")

def analyze_dataset(flights: List[Dict[str, Any]]):
    """Analyze the generated dataset and print statistics."""
    total_flights = len(flights)
    
    # Status distribution
    status_counts = {}
    for flight in flights:
        status = flight["status"]
        status_counts[status] = status_counts.get(status, 0) + 1
    
    # Delay distribution
    delays = [flight["delay_minutes"] for flight in flights if not flight["cancelled"]]
    
    # Weather impact
    weather_delays = {}
    for flight in flights:
        if flight["cancelled"]:
            continue
        weather = flight["weather_condition"]
        if weather not in weather_delays:
            weather_delays[weather] = []
        weather_delays[weather].append(flight["delay_minutes"])
    
    # Airline performance
    airline_delays = {}
    for flight in flights:
        if flight["cancelled"]:
            continue
        airline = flight["airline_code"]
        if airline not in airline_delays:
            airline_delays[airline] = []
        airline_delays[airline].append(flight["delay_minutes"])
    
    # Print statistics
    print("\n=== Flight Dataset Statistics ===")
    print(f"Total flights: {total_flights}")
    
    print("\nStatus Distribution:")
    for status, count in status_counts.items():
        percentage = (count / total_flights) * 100
        print(f"  {status}: {count} ({percentage:.1f}%)")
    
    if delays:
        print("\nDelay Statistics:")
        print(f"  Average delay: {sum(delays) / len(delays):.1f} minutes")
        print(f"  Median delay: {sorted(delays)[len(delays)//2]} minutes")
        print(f"  Max delay: {max(delays)} minutes")
        
        # Calculate delay buckets
        delay_buckets = {
            "0-15 min": len([d for d in delays if 0 <= d < 15]),
            "15-30 min": len([d for d in delays if 15 <= d < 30]),
            "30-60 min": len([d for d in delays if 30 <= d < 60]),
            "60-120 min": len([d for d in delays if 60 <= d < 120]),
            "120-180 min": len([d for d in delays if 120 <= d < 180]),
            "180+ min": len([d for d in delays if d >= 180])
        }
        
        print("\nDelay Distribution:")
        for bucket, count in delay_buckets.items():
            percentage = (count / len(delays)) * 100
            print(f"  {bucket}: {count} ({percentage:.1f}%)")
    
    print("\nWeather Impact:")
    for weather, delays in weather_delays.items():
        if delays:
            avg_delay = sum(delays) / len(delays)
            print(f"  {weather}: avg delay {avg_delay:.1f} minutes, {len(delays)} flights")
    
    print("\nAirline Performance:")
    for airline, delays in airline_delays.items():
        if delays:
            avg_delay = sum(delays) / len(delays)
            print(f"  {airline}: avg delay {avg_delay:.1f} minutes, {len(delays)} flights")

def visualize_dataset(flights: List[Dict[str, Any]], output_dir: str):
    """Create visualizations of the dataset."""
    if not os.path.exists(output_dir):
        os.makedirs(output_dir)
    
    # Status distribution pie chart
    status_counts = {}
    for flight in flights:
        status = flight["status"]
        status_counts[status] = status_counts.get(status, 0) + 1
    
    plt.figure(figsize=(10, 6))
    labels = list(status_counts.keys())
    sizes = list(status_counts.values())
    plt.pie(sizes, labels=labels, autopct='%1.1f%%', startangle=90)
    plt.axis('equal')
    plt.title('Flight Status Distribution')
    plt.savefig(os.path.join(output_dir, 'status_distribution.png'))
    plt.close()
    
    # Delay histogram
    delays = [flight["delay_minutes"] for flight in flights if not flight["cancelled"]]
    
    plt.figure(figsize=(10, 6))
    plt.hist(delays, bins=range(0, max(delays) + 30, 30), edgecolor='black')
    plt.xlabel('Delay Minutes')
    plt.ylabel('Number of Flights')
    plt.title('Flight Delay Distribution')
    plt.grid(axis='y', alpha=0.75)
    plt.savefig(os.path.join(output_dir, 'delay_histogram.png'))
    plt.close()
    
    # Weather impact
    weather_delays = {}
    for flight in flights:
        if flight["cancelled"]:
            continue
        weather = flight["weather_condition"]
        if weather not in weather_delays:
            weather_delays[weather] = []
        weather_delays[weather].append(flight["delay_minutes"])
    
    weather_avg_delays = {w: sum(d)/len(d) if d else 0 for w, d in weather_delays.items()}
    
    plt.figure(figsize=(10, 6))
    weather_names = list(weather_avg_delays.keys())
    avg_delays = list(weather_avg_delays.values())
    plt.bar(weather_names, avg_delays)
    plt.xlabel('Weather Condition')
    plt.ylabel('Average Delay (minutes)')
    plt.title('Weather Impact on Flight Delays')
    plt.xticks(rotation=45)
    plt.tight_layout()
    plt.savefig(os.path.join(output_dir, 'weather_impact.png'))
    plt.close()
    
    # Airline performance
    airline_delays = {}
    for flight in flights:
        if flight["cancelled"]:
            continue
        airline = flight["airline_code"]
        if airline not in airline_delays:
            airline_delays[airline] = []
        airline_delays[airline].append(flight["delay_minutes"])
    
    airline_avg_delays = {a: sum(d)/len(d) if d else 0 for a, d in airline_delays.items()}
    
    plt.figure(figsize=(10, 6))
    airline_codes = list(airline_avg_delays.keys())
    avg_delays = list(airline_avg_delays.values())
    
    # Sort by average delay
    sorted_indices = np.argsort(avg_delays)
    sorted_airlines = [airline_codes[i] for i in sorted_indices]
    sorted_delays = [avg_delays[i] for i in sorted_indices]
    
    plt.bar(sorted_airlines, sorted_delays)
    plt.xlabel('Airline')
    plt.ylabel('Average Delay (minutes)')
    plt.title('Airline Performance Comparison')
    plt.savefig(os.path.join(output_dir, 'airline_performance.png'))
    plt.close()
    
    print(f"Visualizations saved to {output_dir}")

def main():
    parser = argparse.ArgumentParser(description='Generate mock flight data for testing')
    parser.add_argument('--start-date', type=str, default=datetime.datetime.now().strftime("%Y-%m-%d"),
                        help='Start date for data generation (YYYY-MM-DD)')
    parser.add_argument('--days', type=int, default=30,
                        help='Number of days to generate data for')
    parser.add_argument('--flights-per-day', type=int, default=100,
                        help='Number of flights to generate per day')
    parser.add_argument('--output-json', type=str, default='flight_data.json',
                        help='Output JSON file path')
    parser.add_argument('--output-csv', type=str, default='flight_data.csv',
                        help='Output CSV file path')
    parser.add_argument('--visualize', action='store_true',
                        help='Generate data visualizations')
    parser.add_argument('--vis-dir', type=str, default='visualizations',
                        help='Directory for visualizations')
    
    args = parser.parse_args()
    
    # Parse start date
    start_date = datetime.datetime.strptime(args.start_date, "%Y-%m-%d")
    
    # Generate data
    flights = generate_dataset(start_date, args.days, args.flights_per_day)
    
    # Save data
    save_json(flights, args.output_json)
    save_csv(flights, args.output_csv)
    
    # Analyze data
    analyze_dataset(flights)
    
    # Visualize if requested
    if args.visualize:
        visualize_dataset(flights, args.vis_dir)

if __name__ == "__main__":
    main()