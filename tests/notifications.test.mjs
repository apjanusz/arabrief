import test from "node:test";
import assert from "node:assert/strict";
import { createNotificationStore } from "../src/app/notifications/notifications.ts";

test("independent identical actions produce separate notifications", () => {
  const store = createNotificationStore();
  const first = store.success("Zapisano zmiany.");
  const second = store.success("Zapisano zmiany.");
  assert.notEqual(first, second);
  assert.deepEqual(store.getSnapshot().map(item => item.id), [first, second]);
});

test("background failures deduplicate without resetting the snapshot, changed errors update", () => {
  const store = createNotificationStore();
  const id = store.error("Brak połączenia", { key: "player" });
  const before = store.getSnapshot();
  for (let i = 0; i < 20; i++) assert.equal(store.error("Brak połączenia", { key: "player" }), id);
  assert.equal(store.getSnapshot(), before);
  assert.equal(store.error("Inny błąd", { key: "player" }), id);
  assert.equal(store.getSnapshot().length, 1);
  assert.equal(store.getSnapshot()[0].message, "Inny błąd");
  assert.equal(before[0].message, "Brak połączenia");
});

test("only success and information expire; warnings and errors require dismissal", () => {
  const store = createNotificationStore();
  store.success("OK"); store.info("Anulowano"); store.warning("Uwaga"); store.error("Błąd");
  assert.deepEqual(store.getSnapshot().map(item => item.duration), [5000, 5000, 0, 0]);
});

test("closing is idempotent, retains the exit animation, and cannot remove a newer toast", () => {
  const store = createNotificationStore();
  const id = store.error("Błąd", { key: "player" });
  store.dismiss(id);
  const closing = store.getSnapshot();
  assert.equal(closing[0].closing, true);
  store.dismiss(id);
  assert.equal(store.getSnapshot(), closing);
  const next = store.error("Błąd", { key: "player" });
  assert.notEqual(next, id);
  store.remove(id);
  assert.deepEqual(store.getSnapshot().map(item => item.id), [next]);
});

test("subscription cleanup prevents notifications after unmount", () => {
  const store = createNotificationStore();
  let calls = 0;
  const unsubscribe = store.subscribe(() => calls++);
  store.success("Pierwsza");
  unsubscribe();
  store.info("Druga");
  assert.equal(calls, 1);
});
