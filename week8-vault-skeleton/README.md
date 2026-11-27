# Week 8 — Каркас Vault (Skeleton)

Начало сквозного проекта **Liquidity Vault** (недели 8–15).

## ⚠️ Devnet-деплой
Ожидается на **неделе 14** вместе с финальной версией (неделя 13 код).
Сейчас всё протестировано на **localnet** — методичка разрешает localnet.
Работаем только в тестовых сетях, без реальных денег.

## Аккаунты и владельцы

| Аккаунт | Тип / Seeds | Owner / authority | Что хранит |
|---|---|---|---|
| `vault_state` | PDA `[b"vault_state", token_mint]` | программа | authority, token_mint, share_mint, total_tokens, total_shares |
| `share_mint` | SPL Mint | **mint authority = vault_state PDA** | токен-квитанция |
| `vault_token_account` | SPL Token Account | **owner = vault_state PDA** | внесённые токены |
| `user_position` | PDA `[b"user_position", user]` | программа | owner, amount, shares |

## Действия

1. `initialize_vault(bump)` — создаёт `vault_state`. Share-mint и vault-token-account создаются снаружи (тест) и передаются в инструкцию.
2. `open_position()` — создаёт карточку пользователя (PDA от адреса).

## Ответы на вопросы PK1

**Почему share-токен выпускает программа, а не администратор?**
`mint_authority` у `share_mint` = `vault_state` PDA. У PDA нет приватного ключа → подписать mint может только сама программа через CPI с seeds.

**Что такое PDA и почему у неё нет приватного ключа?**
Program Derived Address. Вычисляется из `seeds + program_id + bump`, не лежит на кривой ed25519 → keypair не существует в принципе.

**Как найти карточку пользователя?**
`PublicKey.findProgramAddressSync([b"user_position", user.toBuffer()], programId)` — детерминированный адрес.

## Доказательство (localnet)

`anchor test --skip-local-validator` → **2 passing**:
- `✔ Initializes the vault`
- `✔ Opens a user position`
