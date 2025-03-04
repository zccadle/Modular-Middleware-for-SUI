module flight_insurance::policy {
    use sui::object::{Self, UID};
    use sui::transfer;
    use sui::tx_context::{Self, TxContext};
    use sui::coin::{Self, Coin};
    use sui::sui::SUI;
    use sui::event;
    use std::string::{Self, String};
    use sui::balance::{Self, Balance};
    use sui::table::{Self, Table};

    // Define insurance policy struct
    struct Policy has key, store {
        id: UID,
        policy_id: String,
        owner: address,
        flight_number: String,
        policy_type: String,
        premium_paid: u64,
        is_claimed: bool,
        expiration_time: u64,
    }

    // Define treasury object to hold funds for payouts
    struct Treasury has key {
        id: UID,
        balance: Balance<SUI>,
        policies: Table<String, Policy>
    }

    // Events
    struct PolicyCreated has copy, drop {
        policy_id: String,
        owner: address,
        flight_number: String,
        policy_type: String,
        premium_paid: u64,
    }

    struct PolicyClaimed has copy, drop {
        policy_id: String,
        owner: address,
        flight_number: String,
        compensation_amount: u64,
        delay_minutes: u64,
    }

    struct ClaimRejected has copy, drop {
        policy_id: String,
        owner: address,
        flight_number: String,
        reason: String,
    }

    // Error codes
    const EInsufficientPayment: u64 = 1;
    const EUnauthorizedOracle: u64 = 2;
    const EPolicyAlreadyClaimed: u64 = 3;
    const EPolicyExpired: u64 = 4;
    const EPolicyNotFound: u64 = 5;
    const EFlightNumberMismatch: u64 = 6;
    const EInsufficientTreasuryBalance: u64 = 7;

    // Constants for authorization
    const MIDDLEWARE_ORACLE: address = @middleware_oracle;

    // Create the treasury - only needs to be done once
    fun init(ctx: &mut TxContext) {
        let treasury = Treasury {
            id: object::new(ctx),
            balance: balance::zero(),
            policies: table::new(ctx)
        };
        
        transfer::share_object(treasury);
    }

    // Fund the treasury
    public entry fun fund_treasury(
        treasury: &mut Treasury,
        payment: &mut Coin<SUI>,
        amount: u64,
        ctx: &mut TxContext
    ) {
        let payment_balance = coin::split(payment, amount, ctx);
        let sui_balance = coin::into_balance(payment_balance);
        balance::join(&mut treasury.balance, sui_balance);
    }

    // Create a new flight insurance policy
    public entry fun create_policy(
        treasury: &mut Treasury,
        payment: &mut Coin<SUI>, 
        policy_id: vector<u8>,
        flight_number: vector<u8>,
        policy_type: vector<u8>,
        premium_amount: u64,
        valid_days: u64,
        ctx: &mut TxContext
    ) {
        // Ensure payment is sufficient
        assert!(coin::value(payment) >= premium_amount, EInsufficientPayment);
        
        // Extract payment and add to treasury
        let policy_payment = coin::split(payment, premium_amount, ctx);
        let sui_balance = coin::into_balance(policy_payment);
        balance::join(&mut treasury.balance, sui_balance);

        // Convert to strings
        let policy_id_str = string::utf8(policy_id);
        let flight_num_str = string::utf8(flight_number);
        let policy_type_str = string::utf8(policy_type);

        // Calculate expiration (current time + valid_days in milliseconds)
        let expiration = tx_context::epoch(ctx) + (valid_days * 24 * 60 * 60 * 1000);
        
        // Create the policy
        let policy = Policy {
            id: object::new(ctx),
            policy_id: policy_id_str,
            owner: tx_context::sender(ctx),
            flight_number: flight_num_str,
            policy_type: policy_type_str,
            premium_paid: premium_amount,
            is_claimed: false,
            expiration_time: expiration,
        };
        
        // Emit event
        event::emit(PolicyCreated {
            policy_id: policy.policy_id,
            owner: policy.owner,
            flight_number: policy.flight_number,
            policy_type: policy.policy_type,
            premium_paid: policy.premium_paid,
        });
        
        // Add policy to treasury's policy table
        table::add(&mut treasury.policies, policy_id_str, policy);
    }

    // Process flight delay claim (called by trusted oracle/middleware)
    public entry fun process_claim(
        treasury: &mut Treasury,
        policy_id: vector<u8>,
        flight_number: vector<u8>,
        delay_minutes: u64, 
        compensation_amount: u64,
        is_cancelled: bool,
        ctx: &mut TxContext
    ) {
        // Convert strings
        let policy_id_str = string::utf8(policy_id);
        let flight_num_str = string::utf8(flight_number);
        
        // Ensure the caller is the authorized middleware oracle
        assert!(tx_context::sender(ctx) == MIDDLEWARE_ORACLE, EUnauthorizedOracle);
        
        // Ensure the policy exists
        assert!(table::contains(&treasury.policies, policy_id_str), EPolicyNotFound);
        
        // Get the policy
        let policy = table::borrow_mut(&mut treasury.policies, policy_id_str);
        
        // Validate policy
        assert!(!policy.is_claimed, EPolicyAlreadyClaimed); // Policy not already claimed
        assert!(policy.expiration_time > tx_context::epoch(ctx), EPolicyExpired); // Policy not expired
        assert!(policy.flight_number == flight_num_str, EFlightNumberMismatch); // Flight number matches
        
        // Check if eligible for claim
        if (delay_minutes >= 30 || is_cancelled) {
            // Ensure treasury has enough balance
            assert!(balance::value(&treasury.balance) >= compensation_amount, EInsufficientTreasuryBalance);
            
            // Mark policy as claimed
            policy.is_claimed = true;
            
            // Send compensation to policy owner
            let payment = coin::take(&mut treasury.balance, compensation_amount, ctx);
            transfer::public_transfer(payment, policy.owner);
            
            // Emit claim event
            event::emit(PolicyClaimed {
                policy_id: policy.policy_id,
                owner: policy.owner,
                flight_number: policy.flight_number,
                compensation_amount,
                delay_minutes,
            });
        } else {
            // Emit rejection event if criteria not met
            event::emit(ClaimRejected {
                policy_id: policy.policy_id,
                owner: policy.owner,
                flight_number: policy.flight_number,
                reason: string::utf8(b"Delay less than 30 minutes"),
            });
        }
    }
}