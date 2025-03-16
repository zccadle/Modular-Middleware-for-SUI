# SUI Modular Middleware: Security Model

## Trust Assumptions

### Integrity for Blockchain
**Trusted**: false

**Justification**: The middleware verifies transaction results with receipts and effects

**Mitigations**:
- Transaction receipt validation
- State verification
- Cross-chain verification for critical transactions

### Liveness for Blockchain
**Trusted**: false

**Justification**: Blockchain networks may experience downtime or congestion

**Mitigations**:
- Network health monitoring
- Multi-chain support for critical operations
- Cached operation replay

### Authentication for Blockchain
**Trusted**: true

**Justification**: Blockchain cryptographic authentication is considered secure

**Mitigations**:
- Use of standard cryptographic primitives
- Regular key rotation for middleware addresses

### Integrity for External Data Source
**Trusted**: false

**Justification**: External APIs may provide incorrect or manipulated data

**Mitigations**:
- Data source redundancy
- Consistency checks across sources
- Cryptographic attestation where available
- Data validation against business rules

### Availability for External Data Source
**Trusted**: false

**Justification**: External APIs may experience downtime or rate limiting

**Mitigations**:
- Circuit breaking pattern
- Local caching with time-based invalidation
- Multiple API providers for critical data

### Integrity for Middleware
**Trusted**: true

**Justification**: The middleware is the trusted component in the system

**Mitigations**:
- Formal verification of critical components
- Comprehensive audit logging
- Runtime verification of security properties

### Availability for Network
**Trusted**: false

**Justification**: Network connections may fail or be interrupted

**Mitigations**:
- Retries with exponential backoff
- Circuit breaking for failing endpoints
- Multiple network paths for critical operations

## Threat Model

### Malicious blockchain node
**Description**: A blockchain node that provides incorrect transaction results

**Category**: Tampering

**Target**: Blockchain

**Affected Properties**:
- Integrity

**Risk**: Likelihood 2 x Impact 5 = 10

**Mitigations**:
- Verification of transaction receipts and effects
- Comparison with results from multiple nodes
- Byzantine fault detection

### Manipulated API data
**Description**: External API provides manipulated data to trigger specific outcomes

**Category**: Tampering

**Target**: External Data Source

**Affected Properties**:
- Integrity

**Risk**: Likelihood 3 x Impact 4 = 12

**Mitigations**:
- Multiple data source verification
- Business logic validation of data ranges
- Anomaly detection for unusual values
- Signed data attestations where available

### API provider outage
**Description**: External API becomes unavailable during critical operations

**Category**: Denial of Service

**Target**: External Data Source

**Affected Properties**:
- Availability

**Risk**: Likelihood 4 x Impact 3 = 12

**Mitigations**:
- Fallback data sources
- Caching of previous responses
- Degraded mode operation
- Retry mechanisms with backoff

### Key compromise
**Description**: Middleware's private keys are compromised

**Category**: Elevation of Privilege

**Target**: Middleware

**Affected Properties**:
- Authentication
- Non-repudiation

**Risk**: Likelihood 2 x Impact 5 = 10

**Mitigations**:
- Hardware security modules for key storage
- Regular key rotation
- Multi-signature requirements for critical operations
- Comprehensive audit logging

### Man-in-the-middle attack
**Description**: Network traffic between middleware and blockchain is intercepted

**Category**: Information Disclosure

**Target**: Network

**Affected Properties**:
- Confidentiality
- Integrity

**Risk**: Likelihood 2 x Impact 4 = 8

**Mitigations**:
- TLS for all communications
- Certificate pinning
- Message signing
- Connection anomaly detection

## Security Guarantees

### VerifiedExecution
**Description**: The middleware verifies that transactions executed on the blockchain produced the expected results, detecting malicious or faulty nodes

**Supported Properties**:
- Integrity
- Safety

**Required Trust Assumptions**:
- Middleware

### BlockchainLivenessTolerance
**Description**: The middleware tolerates temporary blockchain unavailability through caching, retries, and cross-chain fallbacks

**Supported Properties**:
- Availability
- Liveness

**Required Trust Assumptions**:
- Middleware
- Network

### ExternalDataConsistency
**Description**: The middleware ensures consistency of external data through validation, redundancy, and business logic checks

**Supported Properties**:
- Integrity

**Required Trust Assumptions**:
- Middleware

### AuditTrail
**Description**: The middleware provides a non-repudiable audit trail of all security-relevant operations, supporting forensic analysis

**Supported Properties**:
- Non-repudiation
- Integrity

**Required Trust Assumptions**:
- Middleware

### CrossChainPortability
**Description**: The middleware enables transaction portability across multiple blockchains, maintaining security guarantees

**Supported Properties**:
- Availability
- Liveness

**Required Trust Assumptions**:
- Middleware
- Network

## Security Composition

The SUI Modular Middleware achieves unique security properties through careful composition:

1. **Verified Execution**: We inherit the execution correctness of the underlying blockchain,
   but add verification to detect inconsistencies. This creates a stronger integrity property
   than either component alone.

2. **External Data Integration**: We extend blockchain's closed-world model with verified
   external data sources, maintaining integrity through redundancy and consistency checks.

3. **Cross-Chain Resilience**: We inherit security properties from multiple chains,
   allowing the system to maintain liveness even when individual chains experience issues.

4. **Trust Minimization**: Most importantly, our architecture clearly separates trust domains,
   requiring minimal trust in any individual component to maintain overall system security.
   
5. **Clear Security Boundaries**: By explicitly modeling trust assumptions and verification
   mechanisms, we create well-defined security boundaries with provable guarantees.

## Security Delegation with Verification

### Integrity Delegation
**Delegator**: Middleware

**Delegatee**: Blockchain

**Verification**: Transaction receipt and effects verification

**Resulting Guarantee**: VerifiedExecution

### Liveness Delegation
**Delegator**: Middleware

**Delegatee**: Blockchain

**Verification**: Health monitoring and cross-chain fallback

**Resulting Guarantee**: BlockchainLivenessTolerance

### Integrity Delegation
**Delegator**: Middleware

**Delegatee**: External Data Source

**Verification**: Multi-source validation and consistency checks

**Resulting Guarantee**: ExternalDataConsistency

