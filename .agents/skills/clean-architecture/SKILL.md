# SKILL: Clean Code & Architecture

Load this skill when designing modules, services, or reviewing cross-layer structure.

---

## Dependency Rule

Dependencies point **inward only**.

```
[Infra / DB / HTTP / External APIs]
         ↓
[Interface / Controllers / Routes]
         ↓
[Application / Use Cases / Services]
         ↓
[Domain / Entities / Business Rules]
```

- **Domain**: pure business rules. Zero framework imports, zero DB, zero HTTP.
- **Application**: orchestrates domain. Stateless use-cases, no framework coupling.
- **Interface**: thin — validate input shape, call service, return response. Nothing else.
- **Infrastructure**: DB, HTTP clients, queues, file system. All isolated here.

---

## Monorepo Structure

```
/
├── apps/
│   ├── frontend/          # Astro + Vue 3
│   └── backend/           # Bun API
├── libs/
│   ├── ui/                # Shared Vue components
│   ├── shared/            # Shared Zod schemas, types, utils
│   └── db/                # DB clients, migrations, query builders
└── AGENTS.md
```

Zod schemas in `libs/shared` — one validation source for both frontend and backend.

---

## Module Rules

- One responsibility per module, file, class, function.
- File length soft cap: ~200 lines. Exceeded → split by concern.
- Export only what consumers need. Internals stay unexported.
- No barrel `index.ts` re-exports that obscure what's actually imported.
- No circular dependencies.

---

## Functions

- Max ~20 lines. Longer → extract and name the extracted piece.
- Max 3 parameters. More → options object.
- Guard clauses: return early to avoid nesting.
- No boolean parameters — they are two functions disguised as one.

```js
// ❌ Boolean param = hidden branching
const processPayment = (paymentData, isRefund) => { ... }

// ✅ Explicit intent
const chargePayment = (paymentData) => { ... }
const refundPayment = (paymentData) => { ... }
```

---

## Error Handling

```js
const findUserById = (userId) =>
  db.query('SELECT id, email FROM users WHERE id = $1', [userId])
    .then(rows => rows[0] ?? null)
    .catch(dbError => {
      logger.error({ dbError, userId }, 'findUserById failed')
      throw new DatabaseError(`Could not fetch user ${userId}`, { cause: dbError })
    })
```

- Never swallow silently.
- Typed error hierarchy — `AppError → DatabaseError | NotFoundError | AuthError`.
- Global error handler at the HTTP boundary.
- Log original error with context before rethrowing.

---

## What to Avoid

- God modules (one file doing everything).
- Shared mutable state across modules.
- Infrastructure concerns (DB queries, HTTP calls) leaking into domain logic.
- Premature abstractions — YAGNI. Build the simplest thing that works.
- Abstract factories, decorators, design patterns "just in case".