import test from "node:test";
import assert from "node:assert/strict";

const BASE_URL = process.env.GRID_WEB_BASE_URL ?? "http://127.0.0.1:8080";

async function fetchHtml(pathname) {
  const response = await fetch(new URL(pathname, BASE_URL), { redirect: "manual" });
  assert.equal(response.status, 200, `${pathname} should render successfully`);
  return response.text();
}

test("login page follows route locale and keeps the TOTP field available by default", async () => {
  const [zhHtml, enHtml] = await Promise.all([
    fetchHtml("/zh/login"),
    fetchHtml("/en/login"),
  ]);

  assert.match(zhHtml, /name="totpCode"/, "zh login should render the TOTP input by default");
  assert.match(zhHtml, /登录失败|安全基线|登录/, "zh login should keep Chinese copy");
  assert.match(zhHtml, /如果尚未启用 TOTP，可先留空|未启用 TOTP 可留空/, "zh login should explain that first-time users may leave TOTP empty");

  assert.match(enHtml, /name="totpCode"/, "en login should render the TOTP input by default");
  assert.match(enHtml, /Login|Sign in/, "en login should render English login copy");
  assert.match(enHtml, /Leave it blank if TOTP is not enabled yet|If TOTP is not enabled yet, leave it blank/, "en login should explain that first-time users may leave TOTP empty");
  assert.doesNotMatch(enHtml, /登录后进入你的交易控制台|忘记密码？重置密码|还没有账号？去注册/, "en login should not leak Chinese public auth copy");
});

test("register page no longer requires email verification before first login", async () => {
  const [zhHtml, enHtml] = await Promise.all([
    fetchHtml("/zh/register"),
    fetchHtml("/en/register"),
  ]);

  assert.match(zhHtml, /注册后可直接登录|创建账号后可直接登录/, "zh register should explain direct login after registration");
  assert.doesNotMatch(zhHtml, /需要进行验证|必须完成邮箱验证|验证邮箱/, "zh register should not require email verification");

  assert.match(enHtml, /Sign in right after registration|Direct sign-in after registration|Create your account and sign in immediately/, "en register should explain direct login after registration");
  assert.doesNotMatch(enHtml, /Verification required|Email verification required|Verify Email/, "en register should not require email verification");
});
