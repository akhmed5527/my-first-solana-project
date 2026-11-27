# Week 10 — CPI и вывод из vault

Наследует недели 8–9. Добавлен `withdraw` через CPI.

## Program ID
`ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q`
⚠️ Devnet-деплой — на неделе 14. Сейчас localnet.

## Что такое CPI

Cross-Program Invocation — вызов одной программы из другой. Vault сам не двигает токены: он просит Token Program через CPI.

Два случая:
- **Подписывает пользователь** — когда двигаются ЕГО токены (transfer в deposit, burn в withdraw).
- **Подписывает программа от имени PDA** — когда двигаются токены vault (mint в deposit, transfer из vault в withdraw).

## Действие `withdraw(shares_amount)`

1. `tokens_to_return = shares_amount × total_tokens ÷ total_shares`
2. **Burn** share-токенов (authority = user)
3. **Transfer** токенов из vault (authority = vault_state PDA через seeds)
4. Обновляем `total_tokens`, `total_shares`, `user_position`

## CPI-вызовы в транзакции вывода (inner instructions)

1. **`Token Program: Burn`** — сжигание share-токенов, подписано **пользователем**
2. **`Token Program: Transfer`** — перевод из vault пользователю, подписано **vault_state PDA**

## Почему админ не может забрать токены?

`authority` у `vault_token_account` = `vault_state` **PDA**, а не admin. У админа нет приватного ключа PDA.

## Проверки

- `shares_amount > 0`
- `shares_amount <= user_position.shares` (InsufficientShares)
- `total_shares > 0`
- `checked_sub / checked_mul / checked_div`
- `user_position.owner == user.key()` (constraint)

## Тест (localnet)

`anchor test --skip-local-validator` → **5 passing**:
- Initializes the vault
- Opens a user position
- Deposit 100 tokens → 100 shares
- Withdraw 40 shares → 40 tokens back, 40 shares burned
- Rejects withdrawal exceeding user shares
