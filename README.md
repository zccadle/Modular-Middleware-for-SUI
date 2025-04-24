# Suimodular - Byzantine-resilient Middleware Attestation Framework

A decentralized middleware attestation framework with Byzantine fault tolerance for verifying and executing transactions on SUI blockchain.

## Key Features

- **Byzantine-Resilient Quorum Signatures**: Implements a quorum-based signature collection system that can withstand up to 33% Byzantine nodes
- **Formal Security Model**: Based on Byzantine agreement principles with security guarantees formally defined
- **On-chain Verification Contract**: Move language contract deployed on SUI blockchain for verifying middleware attestations
- **Multi-language Execution**: Support for JavaScript and Python code execution within the middleware
- **Performance Optimized**: Off-chain processing with selective on-chain verification for cost efficiency

## Architecture

The system follows a three-layer architecture:

1. **Client Application Layer**: Submits requests through the middleware API
2. **Middleware Framework Layer**:
   - Verifies requests against security policies
   - Collects signatures from a quorum of attestation nodes
   - Submits verified transactions to the blockchain
3. **On-chain Verification Layer**:
   - Validates quorum signatures
   - Executes verified transactions

## Benchmark Results

Performance testing with 100 iterations shows:
- **End-to-End Performance**: 100% success rate, ~1018ms average total iteration time
- **Byzantine Resilience**:
  - 0% Byzantine: 100% success rate
  - 10% Byzantine: 97% success rate
  - 20% Byzantine: 91% success rate
  - 33% Byzantine: 86% success rate (near theoretical BFT limit)
  - 50% Byzantine: 59% success rate 
  - 75% Byzantine: 42% success rate

For detailed benchmark data, see `benchmark_results_100_iter/refactored_benchmarks.json`.

## Setup and Installation

### Prerequisites

- Rust and Cargo
- SUI CLI (for contract deployment)
- Docker (for containerized testing)
- Git

### Building from Source

1. **Clone the repository**:
   ```bash
   git clone https://github.com/your-username/suimodular.git
   cd suimodular
   ```

2. **Build the project**:
   ```bash
   cargo build --release
   ```

3. **Deploy the verification contract**:
   ```bash
   # Navigate to the contract directory
   cd verification_contract
   
   # Compile the contract
   sui move build
   
   # Deploy to SUI testnet
   sui client publish --gas-budget 100000000
   ```

4. **Update configuration**:
   After deployment, update the constants in `src/config.rs` with your deployed contract addresses.

### Docker Setup

For consistent testing and deployment, use the Docker image:

```bash
# Build Docker image
docker build -t sui-benchmark-suite .

# Run benchmarks with Docker
docker run --rm -e SUBMITTER_ADDRESS="<YOUR_SUI_ADDRESS>" -e SUBMITTER_KEYPAIR_BASE64="<BASE64_KEYPAIR>" -v "${PWD}/benchmark_results:/app/results" sui-benchmark-suite --benchmark --output-dir /app/results
```

### Required Environment Variables

Before running the middleware **outside of Docker** you need to make a few variables available in your shell (or a local `.env` file):

| Variable | Purpose | Example |
|----------|---------|---------|
| `SUBMITTER_ADDRESS` | Sui address that will submit on-chain verification txs | `0x<your-address>` |
| `SUBMITTER_KEYPAIR_BASE64` | Base-64 encoded keypair for the above address | `<base64-keypair>` |
| `AVIATIONSTACK_API_KEY` | (Optional) Real-time flight data for the flight-delay demos | `a8f7…56a` |
| `OPENWEATHERMAP_API_KEY` | (Optional) Weather oracle key for the weather-based demo | `<your-key>` |

If the API keys are **not** provided the corresponding demos fall back to simulated data. This is still useful to illustrate the execution flow.

## Usage

### Running Benchmarks (core evaluation)

```bash
cargo run --release -- --benchmark --output-dir benchmark_results_final
```

This command executes the benchmark suite (defaults to **100 iterations** as configured in `src/tools/benchmark_suite.rs`) and stores JSON + Markdown summaries in the specified directory.

### Running Middleware Demos (showcase scenarios)

By default `cargo run --release` (with **no** flags) launches the middleware in *DEMO* mode and executes all showcase examples sequentially:

```bash
cargo run --release
```

Individual demos can be invoked directly via Cargo examples, e.g.

```bash
# Flight delay example only
cargo run --release --example flight_delay

# Enhanced flight-insurance example only
cargo run --release --example enhanced_flight_insurance
```

The demos illustrate:

1. JavaScript & Python cross-language execution and verification
2. Real-time REST oracle integration (weather & flight data)
3. Quorum signature collection and on-chain submission
4. Audit logging / security-model generation

*Note*: On-chain submission will fail if the configured gas coin has insufficient balance. This is expected behaviour in a public testnet setting and can be showcased as "real-world failure handling" in the appendix.

### Setting Up Quorum Configuration

```bash
cargo run --release -- --setup-quorum
```

### Verifying Contract Objects

```bash
cargo run --release -- --verify-contract-objects
```

## Project Structure

- `src/`: Rust source code for the middleware framework
  - `quorum/`: Quorum simulation and signature collection
  - `security/`: Security model and Byzantine detection
  - `transaction/`: Transaction processing and verification
  - `sui/`: SUI blockchain integration
  - `tools/`: Benchmark suite and utilities
- `verification_contract/`: Move language contract for on-chain verification
- `benchmark_results_final/`: Final benchmark results
- `docs/`: Additional documentation and design specs (auto-generated files such as `security-model-generated.md` are committed for reference)

## Appendix & Replication Manual

Detailed step-by-step instructions (installation, contract deployment, demo walkthrough, benchmark interpretation) are provided in `report2/main_revised_appendix_B_system_manual.tex`. The content mirrors the sections above to ensure the PDF and repository stay in sync.

## Security Considerations

The middleware implements Byzantine fault tolerance with a threshold of t < n/3, meaning it can tolerate up to 33% malicious or faulty nodes while maintaining correctness.

Key security properties:
- **Safety**: Valid transactions cannot be forged
- **Liveness**: Valid transactions will eventually be processed
- **Byzantine Resilience**: System functions correctly despite malicious nodes

## Future Work

- Production deployment on SUI mainnet
- Integration with decentralized identity frameworks
- Support for additional blockchains beyond SUI
- Enhanced Byzantine detection mechanisms

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Phillip Jovanovic / University College London / Professor of Information Security
- Alberto Sonnino / Mysten Labs / Research Scientist
