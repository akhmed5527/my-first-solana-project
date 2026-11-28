# Week 13 — Позиция Vault в AMM-пуле + сбор комиссий

## Что это
Vault становится провайдером ликвидности: открывает позицию в своём constant-product AMM (x·y=k), кладёт в неё оба токена, получает LP-токены. Пока позиция открыта, каждый обмен в пуле увеличивает стоимость LP — это и есть «заработок» vault. Администратор может собрать часть (частичный `remove_liquidity`) или закрыть позицию полностью. Перед любым выводом пользователей админ обязан закрыть позицию — иначе `withdraw` вернёт `PositionOpen`.

## Program IDs (localnet)

| Программа | ID |
|---|---|
| Vault (`week8_liquidity_vault`) | `ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q` |
| AMM (`simple_amm`) | `CP2fNTqk9F3D6jGrfuue67Rxf4zHGeJtxsHABnTgLBmk` |

Devnet-деплой — неделя 14.

## Схема аккаунтов

### VaultState — расширен относительно недели 12
Новые поля:
- `vault_lp_account: Pubkey` — SPL token account с LP-токенами AMM (authority = vault_state PDA)
- `position_open: bool` — true, пока средства в AMM (блокирует `withdraw`)
- `initial_a: u64`, `initial_b: u64` — сколько A и B было вложено в позицию (для расчёта собранного / перерасчёта при частичном снятии)

### Связь с AMM
- `vault_state` PDA = `user` в `simple_amm::add_liquidity` / `remove_liquidity` (подписывает через seeds `[b"vault_state", token_mint, bump]`)
- `vault_token_a` = `user_a`, `vault_token_b` = `user_b`
- `vault_lp_account` = `user_lp`
- `amm_pool` = `pool`, `amm_lp_mint` = `pool.lp_mint`

## Инструкции (все — admin-only)

| Инструкция | CPI | Что делает |
|---|---|---|
| `open_amm_position(amount_a, amount_b, min_lp)` | `simple_amm::add_liquidity` | кладёт A+B в пул, получает LP на `vault_lp_account`, ставит `position_open = true`, сохраняет `initial_a/b` |
| `collect_fees(lp_amount)` | `simple_amm::remove_liquidity` | сжигает часть LP, возвращает пропорциональные A и B в vault. `initial_a/b` пересчитываются пропорционально оставшемуся LP. Позиция остаётся открытой |
| `close_amm_position(min_a, min_b)` | `simple_amm::remove_liquidity` | сжигает **весь** LP, возвращает A и B в vault. Проверяет `delta_a >= min_a` и `delta_b >= min_b` через `reload()`. Ставит `position_open = false` |
| `swap(amount_in, min_amount_out, a_to_b)` | `simple_amm::swap` | из недели 12, admin приводит баланс к нужной пропорции |
| `withdraw(...)` | — | **блокируется**, если `position_open == true` (ошибка `PositionOpen`) |

## Защита
- Все инструкции — только admin (`constraint = vault_state.authority == admin.key()`)
- `open_amm_position` проверяет `!position_open` (нельзя открыть дважды) и `amount_a/b > 0`
- `close_amm_position` проверяет `delta_a >= min_a`, `delta_b >= min_b` → защита от slippage при закрытии
- `collect_fees` проверяет `lp_amount <= vault_lp_account.amount`
- `withdraw` блокируется, пока позиция открыта — пользователи не могут забрать долю до закрытия AMM-позиции
- Все мутабельные аккаунты в контекстах помечены `mut`; крупные контексты (`AmmPositionAccounts`) обёрнуты в `Box<Account<...>>` — иначе Anchor выдаёт stack overflow 4416 > 4096 байт

## Выбранный «диапазон» — адаптация к AMM
Оригинально методичка требует CLMM с ценовым диапазоном. В нашем constant-product AMM **диапазона нет** — ликвидность работает на всём интервале цен. Параметры, которые задаёт admin при открытии:
- `amount_a`, `amount_b` — сколько каждого токена вложить. Их соотношение определяет начальный сдвиг цены.
- `min_lp` — минимум LP, который согласен получить (защита от slippage между отправкой и исполнением tx).

## Тесты (localnet)

```bash
cd week13-vault-liquidity-position
anchor test --validator legacy
```

8 passing:
1. `initializes vault with LP account`
2. `admin opens AMM position: LP minted to vault_lp_account`
3. `rejects withdraw while position_open=true`
4. `external swaps move price: pool reserves change`
5. `admin collects part of fees (partial remove_liquidity)`
6. `admin closes position: both tokens back to vault`
7. `rebalance: close → swap ratio → open new position`
8. `rejects collect_fees when position closed`

## Сценарий (что показывает тест)

1. **Открытие:** admin кладёт 50A + 50B в пул, получает LP на `vault_lp_account`. `position_open = true`, `initial_a = initial_b = 50_000_000`.
2. **Работа:** внешний пользователь делает swap A→B. Резервы пула меняются (reserveA ↑, reserveB ↓). Стоимость LP vault растёт за счёт комиссии 30 bps.
3. **Сбор комиссий:** admin сжигает 10% LP (`collect_fees`), получает пропорциональные A и B обратно в vault. `initial_a/b` пересчитываются, позиция остаётся открытой.
4. **Ребалансировка:** admin делает swap B→A через AMM (сдвигает пропорцию), открывает новую позицию вокруг новой цены. В CLMM это был бы сдвиг диапазона; в constant-product — новое соотношение A:B.
5. **Закрытие:** admin сжигает оставшийся LP, оба токена возвращаются в vault. `position_open = false`, `initial_a/b = 0`. Проверяется `min_a/min_b` через `reload()`.
6. **Защита:** `withdraw` во время открытой позиции возвращает `PositionOpen`; `collect_fees` после закрытия — `PositionNotOpen`.

## Отклонения от методички
- **CLMM с диапазоном → constant-product AMM.** Причины — в корневом README (devnet-краны блокируют IP, Raydium CLMM не собирается под SBPFv3).
- **Позиция = LP-токены, не NFT.** В constant-product AMM позиция представляется балансом LP на `vault_lp_account` (owner = vault_state PDA).
- **Комиссии «встроены» в LP.** В CLMM комиссии собираются отдельной инструкцией; в constant-product они автоматически увеличивают стоимость LP. Поэтому `collect_fees` = частичный `remove_liquidity`.
- **Impermanent loss.** При сильном движении цены стоимость LP может уменьшиться в одном токене и увеличиться в другом. «Заработанные комиссии» = разница между текущей стоимостью LP и вложенным `initial_a/b`. В нашем AMM `collect_fees` не проверяет, что комиссии положительны — просто возвращает долю; проверка «прибыльности» — задача admin / UI.
