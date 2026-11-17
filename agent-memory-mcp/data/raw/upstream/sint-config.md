# SINT Config — КСС (Когнитивно-Символьная Система)

Детерминированная когнитивная среда исполнения: полные ядра ACT-R, SOAR, LIDA,
слои памяти и симбиоз с владельцем. LLM — только периферийный модуль.

## 1. Принципы

- **Механика — это код, не нейросеть.** Все решения, которые меняют состояние
  (селекция, верификация, коммит), выполняет детерминированный рантайм.
- **No-self-verification (AGENTS.md 1.1) enforced mechanically.** У LLM-моста нет
  методов `commit`/`select`; попытка проставить поля `register`/`commit`/
  `selected`/`verified` извне → **Veto** (v3.5).
- **Семантические регистры** (SENSE/FACT/LOGIC/OPINION/CAUSALITY/ACTION):
  регистр выставляет **только рантайм** по типу кандидата; FACT невозможен без
  источника — иначе downgrade до OPINION (hypothesis, conf ≤ 0.3).
- **Всё детерминировано**: каждый модуль имеет `self_test.py` (PASS/FAIL) и
  демо-приёмку с числовыми критериями.

## 2. Архитектура

```
владелец/среда ──> рантайм (выбор, коммит, верификация — детерминированно)
                     ▲
        кандидаты/перцепция (SENSE|FACT|LOGIC) и ТОЛЬКО предложения
                     │
          [v3.5] LLM-мост: гейты format→role→source→confidence, Veto
          [v3.6] симбиоз: trust-петля + калибровка по исходам/ECE
          [v3.4] памяти: SMEM (эпизоды), EPMEM, SVS, utility learning
          [v3.1–3.3] ядра: ACT-R, SOAR, LIDA
```

## 3. Модули

| Версия | Модуль | Содержимое | Приёмка |
|---|---|---|---|
| v3.1 | `kcc/actr/` | chunk store, активация `ln(Σt⁻ᵈ)`, production matcher | fan-effect: времена 2.2/6.6/13.2/22.0 s (логарифм с fanом) |
| v3.2 | `kcc/soar/` | decision cycle, полные preferences, impasse→substate→chunking | ToH n=3: 7 impasses cold → 0 hot, 7 ходов (оптимум); n=4: 15/15 |
| v3.3 | `kcc/lida/` | 5-фазный cognitive cycle, codelets, PAM (hebbian+reward), global workspace | V-Mattie 7×7: еда за 12 циклов; обучение функционально (mean_act 1.686→2.167, ablation) |
| v3.4 | `kcc/memory/` | SMEM, EPMEM, SVS, ACT-R utility learning | maze: path 16→14, traps 1→0, EPMEM 16→122, util 1.10→3.97; 2 конфигурации |
| v3.5 | `kcc/llm/` | контракт §4: role→register гейты, FACT-source, Veto, bridge без commit/select | self-test 8/8, демо 7/7: sourceless→OPINION 0.3, forbidden→Veto |
| v3.6 | `kcc/symbiosis/` | TrustTracker (экспоненциальное сглаживание), Calibrator (4 бина, ECE) | self-test 8/8, демо 7/7: overconfident llm 0.9→0.5, ECE 0.375>0.108 |

### v3.5 — контракт рантайм↔LLM (§4 blueprint)

| Тип кандидата | Регистр (ставит рантайм) | Требование |
|---|---|---|
| percept | SENSE | conf: вес перцепции |
| claim + source | FACT | source обязателен |
| claim без source | OPINION | conf ≤ 0.3 (hypothesis) |
| generate/candidate | LOGIC | предложение, не решение |
| попытка register/commit/selected/verified | **Veto** | отказ кандидата |

### v3.6 — симбиоз

- `TrustTracker`: `trust += lr·(outcome − trust)`; успех → к 1, фейл → к 0;
  монотонен по качеству источника; сходится к аналитическому равновесию
  (для паттерна k/N, lr: `(a−b)/(1−b)`, `a=(1−lr)^(N−k)`, `b=(1−lr)^N`).
- `Calibrator`: 4 бина confidence; accuracy бина = скорректированный confidence;
  `ECE` — ожидаемая ошибка калибровки.

## 4. Демо-приёмки (`kcc/demos/`)

- `fan_effect.py` — ACT-R: время извлечения vs fan (v3.1)
- `tower_of_hanoi.py` — SOAR: импasses→chunking (v3.2)
- `v_mattie.py` — LIDA: агент в сетке, обучение (v3.3)
- `memory_explorer.py` — памяти: лабиринт с тупиками (v3.4)
- `llm_bridge_demo.py` — мост: регистры/Veto/determinism (v3.5)
- `symbiosis_demo.py` — trust/калибровка: 2 источника, ECE (v3.6)

## 5. Тесты

```powershell
# PowerShell, Windows
$env:PYTHONIOENCODING="utf-8"   # кириллица в выводе Python

python bin/sint-regcheck.py        # register-compliance памяти (реальные блоки)
python kcc/soar/self_test.py       # v3.2
python kcc/lida/self_test.py       # v3.3
python kcc/memory/self_test.py     # v3.4
python kcc/llm/self_test.py        # v3.5
python kcc/symbiosis/self_test.py  # v3.6
python kcc/demos/symbiosis_demo.py # приёмка v3.6 (VERDICT: PASS)
```

Прогон всех self-test одной командой:

```powershell
foreach ($p in @("kcc\soar\self_test.py","kcc\lida\self_test.py","kcc\memory\self_test.py","kcc\llm\self_test.py","kcc\symbiosis\self_test.py")) { python $p | Select-Object -Last 1 }
```

## 6. Дорожная карта

- v3.0 blueprint + register-compliance верификатор ✅
- v3.1 ACT-R ✅ · v3.2 SOAR ✅ · v3.3 LIDA ✅ · v3.4 памяти ✅ · v3.5 мост ✅ · v3.6 симбиоз ✅
- **v3.7** — quorum/auditor: независимая верификация решений (кворум из
  детерминированных проверок), аудит trust-сдвигов владельца.
- **v3.8** — интеграция: среда KCC поверх C2/no-state, tick-инвариантность.

## 7. Память и реестры

- `memory/chain.jsonl` + `memory/blocks/*.json` — блоки знаний (регистры,
  hash-chain, provenance). Пишутся скриптами `sint_mem_write*.py`
  (MCP-мост не выдерживает кириллицу в cp1251). Локальная база — в `.gitignore`.
- `bin/sint-regcheck.py` — независимая проверка соответствия блоков регистрам.
- `skills/kcc/SKILL.md` — blueprint КСС (контракт §4, дорожная карта, статус §8).

## 8. Известные ограничения

- **Verification gap**: независимая ревизия кода субагентами (sint-critic/explore)
  временно недоступна (task-агенты возвращают пустые результаты, 5+ попыток).
  Верификация — механическая (self-test + демо-критерии + аналитика). Требуется
  повторная ревизия `kcc/memory/*` и `kcc/llm/*` при работающем task.
- Git на Windows: предупреждения CRLF при коммитах — безвредны.

## 9. История

| Коммит | Что |
|---|---|
| `8cc1598` | v3.6 симбиоз + README |
| `d4e4716` / `ac82e63` | v3.5 docs / feat |
| `46d8544` / `9365f32` | v3.4 docs / feat |
| `0753098` / `2109f51` | v3.3 docs / feat |
| `3ba5150` | v3.2 feat |
| `18681a8` / `5c062fd` / `1150a5a` | v3.1 feat / blueprint / g-tools MCP |