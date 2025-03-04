import json
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
import sys
import os

def load_json_metrics(filename):
    with open(filename, 'r') as f:
        return json.load(f)

def create_performance_bar_chart(metrics_data, output_file):
    """
    Generate a stacked bar chart showing the breakdown of transaction processing time.
    """
    # Group by transaction type
    df_raw = pd.DataFrame(metrics_data)
    
    # Remove any rows with missing timing data
    df = df_raw.dropna(subset=['generation_time_ms', 'sui_time_ms', 'execution_time_ms'])
    
    # Group by transaction type and calculate averages
    df_grouped = df.groupby('transaction_type').agg({
        'generation_time_ms': 'mean',
        'sui_time_ms': 'mean',
        'execution_time_ms': 'mean',
        'total_time_ms': 'mean',
        'transaction_type': 'count'
    }).rename(columns={'transaction_type': 'count'})
    
    # Calculate middleware overhead percentage
    df_grouped['middleware_overhead_pct'] = ((df_grouped['generation_time_ms'] + df_grouped['execution_time_ms']) / 
                                          df_grouped['sui_time_ms'] * 100)
    
    # Create stacked bar chart
    ax = df_grouped[['generation_time_ms', 'sui_time_ms', 'execution_time_ms']].plot(
        kind='bar', 
        stacked=True,
        figsize=(12, 8),
        color=['#3498db', '#2ecc71', '#e74c3c']
    )
    
    # Customize chart
    plt.title('Transaction Processing Time by Type', fontsize=16)
    plt.xlabel('Transaction Type', fontsize=14)
    plt.ylabel('Time (milliseconds)', fontsize=14)
    plt.xticks(rotation=45)
    plt.grid(axis='y', linestyle='--', alpha=0.7)
    
    # Add a legend
    plt.legend(['Generation Time', 'SUI Blockchain Time', 'Execution Time'], 
               loc='upper center', 
               bbox_to_anchor=(0.5, -0.15),
               ncol=3, 
               fontsize=12)
    
    # Add labels above each bar with the overhead percentage
    for i, tx_type in enumerate(df_grouped.index):
        overhead = df_grouped.loc[tx_type, 'middleware_overhead_pct']
        total = df_grouped.loc[tx_type, 'total_time_ms']
        plt.text(i, total + 5, f"Overhead: {overhead:.1f}%", 
                 ha='center', va='bottom', fontsize=12)
    
    # Add sample count to the x-axis labels
    ax.set_xticklabels([f"{idx}\n(n={int(df_grouped.loc[idx, 'count'])})" for idx in df_grouped.index])
    
    # Ensure the figure has enough space at the bottom for the legend
    plt.tight_layout()
    plt.subplots_adjust(bottom=0.2)
    
    # Save the figure
    plt.savefig(output_file)
    plt.close()
    
    print(f"Chart saved to {output_file}")
    
    # Also print summary statistics
    print("\nPerformance Summary:")
    for tx_type in df_grouped.index:
        print(f"\n{tx_type} Transactions (n={int(df_grouped.loc[tx_type, 'count'])}):")
        print(f"  Generation Time: {df_grouped.loc[tx_type, 'generation_time_ms']:.2f} ms")
        print(f"  SUI Time: {df_grouped.loc[tx_type, 'sui_time_ms']:.2f} ms")
        print(f"  Execution Time: {df_grouped.loc[tx_type, 'execution_time_ms']:.2f} ms")
        print(f"  Total Time: {df_grouped.loc[tx_type, 'total_time_ms']:.2f} ms")
        print(f"  Middleware Overhead: {df_grouped.loc[tx_type, 'middleware_overhead_pct']:.2f}%")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python visualize_performance.py <metrics_json_file> <output_image>")
        sys.exit(1)
    
    metrics_file = sys.argv[1]
    output_file = sys.argv[2]
    
    try:
        metrics_data = load_json_metrics(metrics_file)
        create_performance_bar_chart(metrics_data, output_file)
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)