import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("strategy template persistence stores amount mode, futures margin mode, and leverage", () => {
  const migration = fs
    .readdirSync("db/migrations")
    .sort()
    .map((name) => read(`db/migrations/${name}`))
    .join("\n\n");
  const adminRepo = read("crates/shared-db/src/postgres/admin.rs");
  const strategyRepo = read("crates/shared-db/src/postgres/strategy.rs");

  assert.match(migration, /amount_mode/i, "template migration should create amount_mode column");
  assert.match(migration, /futures_margin_mode/i, "template migration should create futures_margin_mode column");
  assert.match(migration, /leverage/i, "template migration should create leverage column");

  for (const source of [adminRepo, strategyRepo]) {
    assert.match(source, /fn template_from_row[\s\S]*try_get::<String, _>\("amount_mode"\)/, "template row parser should read amount_mode from storage");
    assert.match(source, /fn template_from_row[\s\S]*try_get::<Option<String>, _>\("futures_margin_mode"\)/, "template row parser should read futures margin mode from storage");
    assert.match(source, /fn template_from_row[\s\S]*try_get::<Option<i32>, _>\("leverage"\)/, "template row parser should read leverage from storage");
    assert.match(source, /INSERT INTO strategy_templates[\s\S]*amount_mode[\s\S]*futures_margin_mode[\s\S]*leverage/, "template insert SQL should persist the extra futures fields");
    assert.match(source, /UPDATE strategy_templates[\s\S]*amount_mode[\s\S]*futures_margin_mode[\s\S]*leverage/, "template update SQL should persist the extra futures fields");
  }
});
