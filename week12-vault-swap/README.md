# Week 12 — CPI: Vault → Custom AMM (swap)

## Что это
Vault из недели 10 теперь вызывает `simple_amm::swap` через CPI (Cross-Program Invocation) — та же механика, которую методичка описывает для Raydium, но с нашим AMM из недели 11.

## Изменения в Vault (относительно недели 10)

**`VaultState` расширен:**
- `token_b_mint: Pubkey` — mint второго токена
- `vault_token_a: Pubkey` — token-account для A (authority = vault_state PDA)
- `vault_token_b: Pubkey` — token-account для B (authority = vault_state PDA)
- `amm_pool: Pubkey` — адрес AMM-пула

**`initialize_vault` принимает 4 новых параметра:** `token_b_mint`, `vault_token_a`, `vault_token_b`, `amm_pool`.

**Новая инструкция `swap(amount_in, min_amount_out, a_to_b)`** — admin-only. Vault вызывает `simple_amm::swap` через CPI, подписывая от имени `vault_state` PDA (seeds `[b"vault_state", token_mint, bump]`).

## Program IDs (localnet)

| Программа | ID |
|---|---|
| Vault (`week8_liquidity_vault`) | `ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q` |
| AMM (`simple_amm`) | `CP2fNTqk9F3D6jGrfuue67Rxf4zHGeJtxsHABnTgLBmk` |

Devnet-деплой — неделя 14 (по плану курса).

## Схема аккаунтов

### VaultState (PDA `[b"vault_state", token_mint]`)
| Поле | Что хранит |
|---|---|
| authority | админ |
| token_mint | SPL mint токена A |
| token_b_mint | SPL mint токена B |
| share_mint | SPL mint share-токена (mint authority = vault_state PDA) |
| vault_token_a | SPL token account для A (authority = vault_state PDA) |
| vault_token_b | SPL token account для B (authority = vault_state PDA) |
| amm_pool | PDA пула в simple_amm |
| total_tokens | всего A в vault |
| total_shares | всего share-токенов |
| bump | bump |

### UserPosition (PDA `[b"user_position", user]`)
owner, amount, shares, bump

### Связь с AMM
- `vault_state` PDA = `user` в `simple_amm::swap` (подписывает через seeds)
- `vault_token_a` = `user_a`, `vault_token_b` = `user_b`
- `amm_pool` = `pool` в AMM
- `amm_vault_a/b` = PDA vault-аккаунты AMM

## Инструкции

| Инструкция | Кто | Что делает |
|---|---|---|
| `initialize_vault(bump, token_b_mint, vault_token_a, vault_token_b, amm_pool)` | admin | создаёт vault_state, привязывает AMM |
| `open_position` | любой | создаёт user_position (PDA) |
| `deposit(amount)` | user | mint share-токенов, transfer A в vault |
| `withdraw(shares_amount)` | user | burn share-токенов, transfer A из vault |
| `swap(amount_in, min_amount_out, a_to_b)` | admin | CPI → `simple_amm::swap` |

## Защита
- `swap` доступен только admin (`constraint = vault_state.authority == admin.key()`)
- `min_amount_out` защищает от slippage: если AMM вернёт меньше — транзакция откатывается
- CPI подписан vault_state PDA через seeds `[b"vault_state", token_mint, bump]`
- `amm_pool`, `vault_token_a/b`, `amm_vault_a/b` проверяются по `address = vault_state.*` — нельзя подсунуть чужой пул
- Все мутабельные аккаунты помечены `mut` — иначе AMM получит `writable privilege escalated`

## Тесты (localnet)

```bash
cd week12-vault-swap
anchor test --validator legacy
```

5 passing:
1. `initializes vault with AMM pointers`
2. `admin swaps A → B`
3. `admin swaps B → A back`
4. `rejects swap when min_amount_out too big (slippage)`
5. `rejects swap from non-admin`

## Доказательства (localnet, вместо Explorer)

Публичный Explorer для localnet недоступен. Воспроизводится: `anchor test --validator legacy` → tx signatures в логах; проверка локально через `solana confirm <TX> -u localhost` или `solana logs -u localhost`.

- **Успешный swap A → B:** вход 10_000_000, выход ≈9_970_000 B (fee 30 bps), `vault_token_a` ↓ на 10_000_000, `vault_token_b` ↑ на ≈9_970_000
- **Успешный swap B → A:** вход 5_000_000, выход ≈4_985_000 A
- **Отклонённый по slippage:** `min_amount_out = 999_000_000_000` → `SlippageExceeded` (custom error `0x1771` из simple_amm), балансы не меняются
- **Цена пула:** начальные резервы 500 A / 500 B (1:1) → после swap A→B 10 A: 510 A / ≈490.03 B (≈0.96 B за 1 A) → после swap B→A 5 B: ≈504.9 A / ≈495.03 B (≈0.98 B за 1 A)

## Отклонения от методички
Методичка предлагает Raydium CLMM. Используем свой constant-product AMM (см. корневой README → «Отклонения»). Причины: devnet-краны блокируют наш IP, Raydium CLMM не собирается под SBPFv3 на localnet.
