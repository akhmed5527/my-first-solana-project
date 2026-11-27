# Week 9 — Депозит и share-токены

Наследует неделю 8. Добавлен `deposit`.

## Program ID
`ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q`  
⚠️ Devnet-деплой — на неделе 14 (финальная версия). Сейчас localnet.

## Действие `deposit(amount)`

1. Посчитать, сколько share-токенов положено.
2. Перевести токены пользователя в `vault_token_account` (подписывает **пользователь**).
3. Выпустить share-токены на `user_share_account` (подписывает **vault_state PDA**).
4. Обновить `total_tokens`, `total_shares`, `user_position.amount/shares`.

## Правило расчёта

- Первый вкладчик (`total_shares == 0`): `shares = amount` (1:1).
- Следующие: `shares = amount × total_shares ÷ total_tokens`.

Защищает ранних вкладчиков: если vault заработал комиссии, `total_tokens > total_shares`, поздний вкладчик получает меньше share за те же токены.

## Проверки

- `require!(amount > 0, ZeroAmount)`
- `require!(shares_to_mint > 0, ZeroShares)`
- `checked_add / checked_mul`
- `user_position.owner == user.key()`

## Тест (localnet)

`anchor test --skip-local-validator` → **5 passing**:
- ✔ Initializes the vault
- ✔ Opens a user position
- ✔ First deposit: 100 tokens → 100 shares (1:1)
- ✔ Second deposit: 50 tokens → 50 shares
- ✔ Rejects zero-amount deposit

⚠️ Пропорциональный расчёт (`< 1:1` для поздних) проявится на неделе 13, когда vault начнёт получать комиссии.
