use std::fmt;
use serde::{Serialize, Deserialize};

/// Security composition properties for middleware-enhanced blockchain systems
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityProperty {
    /// Integrity of data/transactions
    Integrity,
    /// Authentication of parties
    Authentication,
    /// Non-repudiation of transactions
    NonRepudiation,
    /// Confidentiality of data
    Confidentiality,
    /// Availability of the system
    Availability,
    /// Liveness of the system
    Liveness,
    /// Safety guarantees
    Safety,
    /// Data integrity specifically for external data
    DataIntegrity,
}

impl fmt::Display for SecurityProperty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SecurityProperty::Integrity => write!(f, "Integrity"),
            SecurityProperty::Authentication => write!(f, "Authentication"),
            SecurityProperty::NonRepudiation => write!(f, "Non-repudiation"),
            SecurityProperty::Confidentiality => write!(f, "Confidentiality"),
            SecurityProperty::Availability => write!(f, "Availability"),
            SecurityProperty::Liveness => write!(f, "Liveness"),
            SecurityProperty::Safety => write!(f, "Safety"),
            SecurityProperty::DataIntegrity => write!(f, "Data Integrity"),
        }
    }
}

/// Actors in the system's trust model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrustActor {
    /// The middleware system
    Middleware,
    /// The underlying blockchain (SUI)
    Blockchain,
    /// External data sources (APIs, oracles)
    ExternalDataSource,
    /// End users of the system
    User,
    /// Network infrastructure
    Network,
}

impl fmt::Display for TrustActor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrustActor::Middleware => write!(f, "Middleware"),
            TrustActor::Blockchain => write!(f, "Blockchain"),
            TrustActor::ExternalDataSource => write!(f, "External Data Source"),
            TrustActor::User => write!(f, "User"),
            TrustActor::Network => write!(f, "Network"),
        }
    }
}

/// Trust assumption for a specific security property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustAssumption {
    /// The security property being considered
    pub property: SecurityProperty,
    /// The actor responsible for providing the property
    pub actor: TrustActor,
    /// Whether the actor is trusted for this property
    pub is_trusted: bool,
    /// Justification for the trust assumption
    pub justification: String,
    /// Mitigation strategies if trust is violated
    pub mitigations: Vec<String>,
}

/// Defines a security threat against the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityThreat {
    /// Name of the threat
    pub name: String,
    /// Description of the threat
    pub description: String,
    /// STRIDE category (Spoofing, Tampering, Repudiation, Information disclosure, Denial of service, Elevation of privilege)
    pub stride_category: String,
    /// Target actor of the threat
    pub target: TrustActor,
    /// Affected security properties
    pub affected_properties: Vec<SecurityProperty>,
    /// Likelihood of the threat (1-5)
    pub likelihood: u8,
    /// Impact if the threat is realized (1-5)
    pub impact: u8,
    /// Mitigation strategies
    pub mitigations: Vec<String>,
}

/// Security model for the SUI Modular Middleware
pub struct SecurityModel {
    /// Trust assumptions for different actors and properties
    pub trust_assumptions: Vec<TrustAssumption>,
    /// Security threats identified in the system
    pub threats: Vec<SecurityThreat>,
}

impl SecurityModel {
    /// Create a new security model with the middleware's specific trust model
    pub fn new() -> Self {
        let mut model = Self {
            trust_assumptions: Vec::new(),
            threats: Vec::new(),
        };

        // Initialize with the core trust assumptions of the middleware
        model.initialize_trust_model();
        model.initialize_threat_model();

        model
    }

    /// Initialize the trust model for the middleware
    fn initialize_trust_model(&mut self) {
        // Trust assumptions about the blockchain (SUI)
        self.trust_assumptions.push(TrustAssumption {
            property: SecurityProperty::Integrity,
            actor: TrustActor::Blockchain,
            is_trusted: false, // We verify blockchain's integrity
            justification: "The middleware verifies transaction results with receipts and effects".to_string(),
            mitigations: vec![
                "Transaction receipt validation".to_string(),
                "State verification".to_string(),
                "Cross-chain verification for critical transactions".to_string(),
            ],
        });

        self.trust_assumptions.push(TrustAssumption {
            property: SecurityProperty::Liveness,
            actor: TrustActor::Blockchain,
            is_trusted: false, // We don't fully trust blockchain liveness
            justification: "Blockchain networks may experience downtime or congestion".to_string(),
            mitigations: vec![
                "Network health monitoring".to_string(),
                "Multi-chain support for critical operations".to_string(),
                "Cached operation replay".to_string(),
            ],
        });

        self.trust_assumptions.push(TrustAssumption {
            property: SecurityProperty::Authentication,
            actor: TrustActor::Blockchain,
            is_trusted: true, // We trust blockchain authentication
            justification: "Blockchain cryptographic authentication is considered secure".to_string(),
            mitigations: vec![
                "Use of standard cryptographic primitives".to_string(),
                "Regular key rotation for middleware addresses".to_string(),
            ],
        });

        // Trust assumptions about external data sources
        self.trust_assumptions.push(TrustAssumption {
            property: SecurityProperty::Integrity,
            actor: TrustActor::ExternalDataSource,
            is_trusted: false, // We don't trust external data integrity
            justification: "External APIs may provide incorrect or manipulated data".to_string(),
            mitigations: vec![
                "Data source redundancy".to_string(),
                "Consistency checks across sources".to_string(),
                "Cryptographic attestation where available".to_string(),
                "Data validation against business rules".to_string(),
            ],
        });

        self.trust_assumptions.push(TrustAssumption {
            property: SecurityProperty::Availability,
            actor: TrustActor::ExternalDataSource,
            is_trusted: false, // External sources may be unavailable
            justification: "External APIs may experience downtime or rate limiting".to_string(),
            mitigations: vec![
                "Circuit breaking pattern".to_string(),
                "Local caching with time-based invalidation".to_string(),
                "Multiple API providers for critical data".to_string(),
            ],
        });

        // Trust assumptions about the middleware itself
        self.trust_assumptions.push(TrustAssumption {
            property: SecurityProperty::Integrity,
            actor: TrustActor::Middleware,
            is_trusted: true, // Middleware is trusted for integrity
            justification: "The middleware is the trusted component in the system".to_string(),
            mitigations: vec![
                "Formal verification of critical components".to_string(),
                "Comprehensive audit logging".to_string(),
                "Runtime verification of security properties".to_string(),
            ],
        });

        // Trust assumptions about the network
        self.trust_assumptions.push(TrustAssumption {
            property: SecurityProperty::Availability,
            actor: TrustActor::Network,
            is_trusted: false, // Network may be unreliable
            justification: "Network connections may fail or be interrupted".to_string(),
            mitigations: vec![
                "Retries with exponential backoff".to_string(),
                "Circuit breaking for failing endpoints".to_string(),
                "Multiple network paths for critical operations".to_string(),
            ],
        });
    }

    /// Initialize the threat model for the middleware
    fn initialize_threat_model(&mut self) {
        // Blockchain integrity threats
        self.threats.push(SecurityThreat {
            name: "Malicious blockchain node".to_string(),
            description: "A blockchain node that provides incorrect transaction results".to_string(),
            stride_category: "Tampering".to_string(),
            target: TrustActor::Blockchain,
            affected_properties: vec![SecurityProperty::Integrity],
            likelihood: 2,
            impact: 5,
            mitigations: vec![
                "Verification of transaction receipts and effects".to_string(),
                "Comparison with results from multiple nodes".to_string(),
                "Byzantine fault detection".to_string(),
            ],
        });

        // External data threats
        self.threats.push(SecurityThreat {
            name: "Manipulated API data".to_string(),
            description: "External API provides manipulated data to trigger specific outcomes".to_string(),
            stride_category: "Tampering".to_string(),
            target: TrustActor::ExternalDataSource,
            affected_properties: vec![SecurityProperty::Integrity],
            likelihood: 3,
            impact: 4,
            mitigations: vec![
                "Multiple data source verification".to_string(),
                "Business logic validation of data ranges".to_string(),
                "Anomaly detection for unusual values".to_string(),
                "Signed data attestations where available".to_string(),
            ],
        });

        self.threats.push(SecurityThreat {
            name: "API provider outage".to_string(),
            description: "External API becomes unavailable during critical operations".to_string(),
            stride_category: "Denial of Service".to_string(),
            target: TrustActor::ExternalDataSource,
            affected_properties: vec![SecurityProperty::Availability],
            likelihood: 4,
            impact: 3,
            mitigations: vec![
                "Fallback data sources".to_string(),
                "Caching of previous responses".to_string(),
                "Degraded mode operation".to_string(),
                "Retry mechanisms with backoff".to_string(),
            ],
        });

        // Middleware threats
        self.threats.push(SecurityThreat {
            name: "Key compromise".to_string(),
            description: "Middleware's private keys are compromised".to_string(),
            stride_category: "Elevation of Privilege".to_string(),
            target: TrustActor::Middleware,
            affected_properties: vec![SecurityProperty::Authentication, SecurityProperty::NonRepudiation],
            likelihood: 2,
            impact: 5,
            mitigations: vec![
                "Hardware security modules for key storage".to_string(),
                "Regular key rotation".to_string(),
                "Multi-signature requirements for critical operations".to_string(),
                "Comprehensive audit logging".to_string(),
            ],
        });

        // Network threats
        self.threats.push(SecurityThreat {
            name: "Man-in-the-middle attack".to_string(),
            description: "Network traffic between middleware and blockchain is intercepted".to_string(),
            stride_category: "Information Disclosure".to_string(),
            target: TrustActor::Network,
            affected_properties: vec![SecurityProperty::Confidentiality, SecurityProperty::Integrity],
            likelihood: 2,
            impact: 4,
            mitigations: vec![
                "TLS for all communications".to_string(),
                "Certificate pinning".to_string(),
                "Message signing".to_string(),
                "Connection anomaly detection".to_string(),
            ],
        });
    }

    /// Get all trust assumptions for a specific security property
    pub fn get_assumptions_for_property(&self, property: &SecurityProperty) -> Vec<&TrustAssumption> {
        self.trust_assumptions.iter()
            .filter(|assumption| std::mem::discriminant(&assumption.property) == std::mem::discriminant(property))
            .collect()
    }

    /// Get all trust assumptions for a specific actor
    pub fn get_assumptions_for_actor(&self, actor: &TrustActor) -> Vec<&TrustAssumption> {
        self.trust_assumptions.iter()
            .filter(|assumption| std::mem::discriminant(&assumption.actor) == std::mem::discriminant(actor))
            .collect()
    }

    /// Get all threats affecting a specific security property
    pub fn get_threats_for_property(&self, property: &SecurityProperty) -> Vec<&SecurityThreat> {
        self.threats.iter()
            .filter(|threat| threat.affected_properties.iter()
                .any(|p| std::mem::discriminant(p) == std::mem::discriminant(property)))
            .collect()
    }

    /// Get all threats targeting a specific actor
    pub fn get_threats_for_actor(&self, actor: &TrustActor) -> Vec<&SecurityThreat> {
        self.threats.iter()
            .filter(|threat| std::mem::discriminant(&threat.target) == std::mem::discriminant(actor))
            .collect()
    }

    /// Get high-risk threats (high likelihood and impact)
    pub fn get_high_risk_threats(&self) -> Vec<&SecurityThreat> {
        self.threats.iter()
            .filter(|threat| threat.likelihood >= 3 && threat.impact >= 4)
            .collect()
    }
}

/// Security guarantees provided by the middleware system
/// These are the formal properties that can be demonstrated through the design
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityGuarantee {
    /// Middleware verifies blockchain transaction execution
    VerifiedExecution,
    /// System tolerates blockchain unavailability through fallbacks
    BlockchainLivenessTolerance,
    /// System ensures external data consistency across sources
    ExternalDataConsistency,
    /// System provides non-repudiable audit trails
    AuditTrail,
    /// System provides transaction portability across chains
    CrossChainPortability,
}

impl SecurityGuarantee {
    /// Get a description of this security guarantee
    pub fn description(&self) -> String {
        match self {
            SecurityGuarantee::VerifiedExecution => 
                "The middleware verifies that transactions executed on the blockchain produced the expected results, \
                 detecting malicious or faulty nodes".to_string(),
            SecurityGuarantee::BlockchainLivenessTolerance => 
                "The middleware tolerates temporary blockchain unavailability through caching, retries, \
                 and cross-chain fallbacks".to_string(),
            SecurityGuarantee::ExternalDataConsistency => 
                "The middleware ensures consistency of external data through validation, redundancy, \
                 and business logic checks".to_string(),
            SecurityGuarantee::AuditTrail => 
                "The middleware provides a non-repudiable audit trail of all security-relevant operations, \
                 supporting forensic analysis".to_string(),
            SecurityGuarantee::CrossChainPortability => 
                "The middleware enables transaction portability across multiple blockchains, \
                 maintaining security guarantees".to_string(),
        }
    }

    /// Get the security properties this guarantee supports
    pub fn supported_properties(&self) -> Vec<SecurityProperty> {
        match self {
            SecurityGuarantee::VerifiedExecution => 
                vec![SecurityProperty::Integrity, SecurityProperty::Safety],
            SecurityGuarantee::BlockchainLivenessTolerance => 
                vec![SecurityProperty::Availability, SecurityProperty::Liveness],
            SecurityGuarantee::ExternalDataConsistency => 
                vec![SecurityProperty::Integrity],
            SecurityGuarantee::AuditTrail => 
                vec![SecurityProperty::NonRepudiation, SecurityProperty::Integrity],
            SecurityGuarantee::CrossChainPortability => 
                vec![SecurityProperty::Availability, SecurityProperty::Liveness],
        }
    }

    /// Get trust assumptions required for this guarantee
    pub fn required_trust_assumptions(&self) -> Vec<TrustActor> {
        match self {
            SecurityGuarantee::VerifiedExecution => 
                vec![TrustActor::Middleware],
            SecurityGuarantee::BlockchainLivenessTolerance => 
                vec![TrustActor::Middleware, TrustActor::Network],
            SecurityGuarantee::ExternalDataConsistency => 
                vec![TrustActor::Middleware],
            SecurityGuarantee::AuditTrail => 
                vec![TrustActor::Middleware],
            SecurityGuarantee::CrossChainPortability => 
                vec![TrustActor::Middleware, TrustActor::Network],
        }
    }
}

/// Document how the middleware system composes security guarantees
pub fn document_security_composition() -> String {
    // This would be expanded significantly in a real implementation
    "The SUI Modular Middleware achieves unique security properties through careful composition:

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
   mechanisms, we create well-defined security boundaries with provable guarantees.".to_string()
}

/// Formal definition of security delegation with verification
pub struct SecurityDelegationWithVerification {
    /// Security property being delegated
    pub property: SecurityProperty,
    /// Actor delegating the property
    pub delegator: TrustActor,
    /// Actor to which the property is delegated
    pub delegatee: TrustActor,
    /// Verification mechanism used by the delegator
    pub verification_mechanism: String,
    /// Security guarantee resulting from the delegation
    pub resulting_guarantee: SecurityGuarantee,
}

/// Document the security delegation pattern used in the middleware
pub fn document_security_delegation() -> Vec<SecurityDelegationWithVerification> {
    vec![
        SecurityDelegationWithVerification {
            property: SecurityProperty::Integrity,
            delegator: TrustActor::Middleware,
            delegatee: TrustActor::Blockchain,
            verification_mechanism: "Transaction receipt and effects verification".to_string(),
            resulting_guarantee: SecurityGuarantee::VerifiedExecution,
        },
        SecurityDelegationWithVerification {
            property: SecurityProperty::Liveness,
            delegator: TrustActor::Middleware,
            delegatee: TrustActor::Blockchain,
            verification_mechanism: "Health monitoring and cross-chain fallback".to_string(),
            resulting_guarantee: SecurityGuarantee::BlockchainLivenessTolerance,
        },
        SecurityDelegationWithVerification {
            property: SecurityProperty::Integrity,
            delegator: TrustActor::Middleware,
            delegatee: TrustActor::ExternalDataSource,
            verification_mechanism: "Multi-source validation and consistency checks".to_string(),
            resulting_guarantee: SecurityGuarantee::ExternalDataConsistency,
        },
    ]
}

/// Example use of the security model for documentation
pub fn generate_security_documentation() -> String {
    let model = SecurityModel::new();
    
    let mut doc = String::new();
    doc.push_str("# SUI Modular Middleware: Security Model\n\n");
    
    // Document trust assumptions
    doc.push_str("## Trust Assumptions\n\n");
    for assumption in &model.trust_assumptions {
        doc.push_str(&format!("### {} for {}\n", assumption.property, assumption.actor));
        doc.push_str(&format!("**Trusted**: {}\n\n", assumption.is_trusted));
        doc.push_str(&format!("**Justification**: {}\n\n", assumption.justification));
        doc.push_str("**Mitigations**:\n");
        for mitigation in &assumption.mitigations {
            doc.push_str(&format!("- {}\n", mitigation));
        }
        doc.push_str("\n");
    }
    
    // Document threats
    doc.push_str("## Threat Model\n\n");
    for threat in &model.threats {
        doc.push_str(&format!("### {}\n", threat.name));
        doc.push_str(&format!("**Description**: {}\n\n", threat.description));
        doc.push_str(&format!("**Category**: {}\n\n", threat.stride_category));
        doc.push_str(&format!("**Target**: {}\n\n", threat.target));
        doc.push_str("**Affected Properties**:\n");
        for property in &threat.affected_properties {
            doc.push_str(&format!("- {}\n", property));
        }
        doc.push_str("\n");
        doc.push_str(&format!("**Risk**: Likelihood {} x Impact {} = {}\n\n", 
            threat.likelihood, threat.impact, threat.likelihood * threat.impact));
        doc.push_str("**Mitigations**:\n");
        for mitigation in &threat.mitigations {
            doc.push_str(&format!("- {}\n", mitigation));
        }
        doc.push_str("\n");
    }
    
    // Document security guarantees
    doc.push_str("## Security Guarantees\n\n");
    let guarantees = vec![
        SecurityGuarantee::VerifiedExecution,
        SecurityGuarantee::BlockchainLivenessTolerance,
        SecurityGuarantee::ExternalDataConsistency,
        SecurityGuarantee::AuditTrail,
        SecurityGuarantee::CrossChainPortability,
    ];
    
    for guarantee in guarantees {
        doc.push_str(&format!("### {:?}\n", guarantee));
        doc.push_str(&format!("**Description**: {}\n\n", guarantee.description()));
        doc.push_str("**Supported Properties**:\n");
        for property in guarantee.supported_properties() {
            doc.push_str(&format!("- {}\n", property));
        }
        doc.push_str("\n");
        doc.push_str("**Required Trust Assumptions**:\n");
        for actor in guarantee.required_trust_assumptions() {
            doc.push_str(&format!("- {}\n", actor));
        }
        doc.push_str("\n");
    }
    
    // Document security composition
    doc.push_str("## Security Composition\n\n");
    doc.push_str(&document_security_composition());
    doc.push_str("\n\n");
    
    // Document security delegation
    doc.push_str("## Security Delegation with Verification\n\n");
    for delegation in document_security_delegation() {
        doc.push_str(&format!("### {} Delegation\n", delegation.property));
        doc.push_str(&format!("**Delegator**: {}\n\n", delegation.delegator));
        doc.push_str(&format!("**Delegatee**: {}\n\n", delegation.delegatee));
        doc.push_str(&format!("**Verification**: {}\n\n", delegation.verification_mechanism));
        doc.push_str(&format!("**Resulting Guarantee**: {:?}\n\n", delegation.resulting_guarantee));
    }
    
    doc
}