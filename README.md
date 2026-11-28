# Liquidity Vault — сквозной проект (недели 8–13)

Учебный DeFi-проект на Solana: **vault с share-токенами** + **собственный constant-product AMM**. Разрабатывается понедельно, каждая неделя — своя папка `weekN-*/` с независимым Anchor-проектом.

Проект реализует механики настоящих DeFi-продуктов (Kamino, Meteora, Yearn): share-токены, CPI, liquidity provision, сбор комиссий, ребалансировка.

## Что делает проект

**Vault** принимает токены пользователей в общий «котёл» и выдаёт **share-токены** — квитанции о доле. Средствами управляет администратор: обменивает через AMM, размещает в liquidity-позиции, собирает комиссии. Пользователь при выходе возвращает share-токены и забирает долю.

**AMM** — constant-product пул (x·y=k) с комиссией 0.3%, LP-токенами, add/swap/remove. Полностью реализован на localnet, заменяет Raydium CLMM (см. «Отклонения»).

## Структура репозитория

| Папка | Что внутри | Тесты |
|---|---|---|
| `week1-proof.md` | доказательство установки toolchain | — |
| `week2-analysis.md` | анализ блокчейн-платформ | — |
| `week3-rust-mini-project/` | вводный Rust-проект | — |
| `week4-proof.md` | доказательство запуска валидатора | — |
| `week5-counter-program/` | Anchor counter | ✅ |
| `week6-pda-profile/` | PDA profile | ✅ |
| `week7-token.md` | SPL Token (создание, минт, transfer) | — |
| `week8-vault-skeleton/` | каркас vault (PDA, share_mint, user_position) | ✅ |
| `week9-vault-deposit/` | депозит и share-токены (1:1 → пропорционально) | ✅ |
| `week10-vault-withdraw/` | вывод через CPI (Burn + Transfer от PDA) | ✅ |
| `week11-simple-amm/` | свой AMM x·y=k (pool, lp_mint, add/swap/remove) | ✅ |
| `week12-vault-swap/` | vault ↔ AMM через CPI (swap A→B и B→A) | **5 passing** |
| `week13-vault-liquidity-position/` | позиция vault в AMM, сбор комиссий, закрытие, ребалансировка | **8 passing** |

**Итого:** 6 недель кода, 13+ тестов, все проходят (`anchor test --validator legacy`).

## Стек

- **Anchor** 1.2.2
- **Solana** 4.4.1 (Agave)
- **Rust** 1.89 (edition 2021)
- **SPL Token** + TypeScript-тесты через `ts-mocha`
- **Валидатор:** `solana-test-validator` (Anchor 1.x через `--validator legacy`; Surfpool не установлен)

## Program IDs (localnet, стабильные с недели 8)

| Программа | ID |
|---|---|
| Vault (`week8_liquidity_vault`) | `ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q` |
| AMM (`simple_amm`) | `CP2fNTqk9F3D6jGrfuue67Rxf4zHGeJtxsHABnTgLBmk` |

ID одинаковые во всех неделях, потому что каждая неделя — эволюция того же кода (vault и AMM доделываются итеративно).

## Как запустить любую неделю

```bash
cd weekN-*/
anchor build
anchor test --validator legacy
```

Каждый проект независим: свой `Anchor.toml`, свой набор тестов, свои keypair-файлы в `target/deploy/`.

## Архитектура vault

### Аккаунты

- **VaultState** (PDA `[b"vault_state", token_mint]`) — настройки: admin, mint A/B, share_mint, vault_token_a/b, vault_lp_account, amm_pool, totals, флаг `position_open`
- **UserPosition** (PDA `[b"user_position", user]`) — сколько внёс / сколько shares
- **vault_token_a/b** — SPL token-аккаунты (authority = vault_state PDA)
- **vault_lp_account** — SPL LP-аккаунт (authority = vault_state PDA)
- **share_mint** — SPL mint (mint authority = vault_state PDA)

### Ключевые принципы

- **Программа владеет деньгами через PDA.** Никто (включая admin) не может забрать токены своей подписью — только программа через seeds.
- **Share-токены — обычные SPL.** Их можно передать, использовать в других протоколах.
- **CPI для всех внешних действий.** Токены двигаются через Token Program, swap/liquidity — через AMM.
- **Пропорциональный учёт.** Вклад позднего пользователя даёт меньше shares, если vault уже заработал комиссии — ранние не «размываются».

## Отклонения от методички (важно для защиты)

| Неделя | Требование | Что сделано | Почему |
|---|---|---|---|
| 11 | Raydium CLMM пул в devnet | Собственный constant-product AMM на localnet | (1) devnet-краны (solfaucet.com, QuickNode, faucet.xyz, Helius, Alchemy, faucet.solana.com) блокируют наш IP → не получить SOL для создания токенов и пула. (2) Raydium CLMM не собирается под SBPFv3 (наш solana-test-validator 4.4.1 требует SBPFv3, SIMD-0500), а под старую арх. (`--arch v0`) выдаёт `Entrypoint out of bounds`. |
| 12 | Swap через Raydium | CPI vault → `simple_amm::swap` | Следствие недели 11 |
| 13 | Позиция в CLMM с диапазоном | LP-позиция в constant-product AMM без диапазона | Следствие недели 11; «комиссии» в AMM автоматически увеличивают стоимость LP, поэтому collect_fees = частичный remove_liquidity |
| 8–13 | Деплой в devnet | Тесты в localnet | Devnet-краны блокируют IP. В README каждой недели есть раздел «Доказательства» с tx-логами из localnet. |

**Все механики, требуемые методичкой на неделях 11–13** (создание пула, добавление ликвидности, swap, позиция, сбор комиссий, ребалансировка) — реализованы и покрыты тестами. Отклонения затрагивают только конкретную реализацию (Raydium → свой AMM) и тестовую сеть (devnet → localnet), но не логику задания.

## Безопасность

- **Никаких ключей и seed-фраз в репозитории** — `.gitignore` блокирует `target/`, `node_modules/`, `*-keypair.json`, `.key`, `.env`
- **Работа только в localnet** (тестовые сети, без реальных денег)
- **Все админо-операции защищены constraint'ом** `vault_state.authority == admin.key()`

## План на недели 14–16

| Неделя | Что планируется |
|---|---|
| 14 (capstone) | Веб-интерфейс (Phantom), devnet-деплой (если краны разблокируются), корневой README |
| 15 (Demo Day) | Живая демонстрация: подключить кошелёк → внести → swap → открыть позицию → собрать комиссии → закрыть → вывести |
| 16 | Индивидуальная защита: каждый объясняет свою часть и отвечает на вопросы |

## Ссылки

- [Solana Docs](https://solana.com/docs)
- [Anchor Docs](https://anchor-lang.com/docs)
- [SPL Token](https://spl.solana.com/token)
