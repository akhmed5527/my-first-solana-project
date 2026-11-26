use std::env;

// Struct (как в Solana Accounts)
#[derive(Debug)]
struct Wallet {
    address: String,
    balance: u64, // lamports
}

// Enum (как в Solana Instructions)
#[derive(Debug)]
enum Instruction {
    Transfer { to: String, amount: u64 },
    Mint { amount: u64 },
}

// Функция с обработкой ошибок (Result)
fn process_instruction(wallet: &mut Wallet, instruction: Instruction) -> Result<(), String> {
    match instruction {
        Instruction::Transfer { to, amount } => {
            if wallet.balance < amount {
                return Err(String::from("Insufficient funds"));
            }
            wallet.balance -= amount;
            println!("✅ Transferred {} lamports to {}", amount, to);
            Ok(())
        }
        Instruction::Mint { amount } => {
            wallet.balance += amount;
            println!("✨ Minted {} lamports", amount);
            Ok(())
        }
    }
}

fn main() {
    // Получаем аргумент из командной строки
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("❌ Usage: cargo run <transfer|mint>");
        return;
    }

    let mut my_wallet = Wallet {
        address: String::from("EQQJfUiEwcxn8ThJf7Jvo8dPGXQ3ghQK5UzhJ2xcmGdh"),
        balance: 1000,
    };

    println!("Initial Wallet: {:?}", my_wallet);

    // Выбор инструкции в зависимости от аргумента
    let instruction = match args[1].as_str() {
        "transfer" => Instruction::Transfer { 
            to: String::from("FriendWalletAddress"), 
            amount: 500 
        },
        "mint" => Instruction::Mint { amount: 5000 },
        _ => {
            println!("Unknown command");
            return;
        }
    };

    println!("Executing Instruction: {:?}", instruction);

    // Вызов функции с обработкой Result
    if let Err(e) = process_instruction(&mut my_wallet, instruction) {
        eprintln!("Error: {}", e);
    }

    println!("Final Wallet: {:?}", my_wallet);
}
