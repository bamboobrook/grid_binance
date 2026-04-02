import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

export default async function AdminTemplatesPage() {
  const state = await getCurrentAdminProductState();

  return (
    <>
      {state.flash.templates ? (
        <StatusBanner description={state.flash.templates.description} title={state.flash.templates.title} tone={state.flash.templates.tone} />
      ) : null}
      <AppShellSection
        description="Create templates, keep them in draft until review completes, and publish them without mutating previously copied user strategies."
        eyebrow="Strategy templates"
        title="Template Management"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Create template</CardTitle>
              <CardDescription>New templates start as drafts and are published only after operator review.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/admin/templates" method="post">
                <input name="intent" type="hidden" value="create" />
                <Field label="Template name">
                  <Input name="name" placeholder="ADA Trend Rider" />
                </Field>
                <Field label="Market">
                  <Select defaultValue="spot" name="market">
                    <option value="spot">Spot</option>
                    <option value="usd-m">USDⓈ-M</option>
                    <option value="coin-m">COIN-M</option>
                  </Select>
                </Field>
                <Field label="Strategy mode">
                  <Select defaultValue="classic" name="mode">
                    <option value="classic">Classic</option>
                    <option value="buy-only">Buy-only</option>
                    <option value="sell-only">Sell-only</option>
                    <option value="long">Long</option>
                    <option value="short">Short</option>
                    <option value="neutral">Neutral</option>
                  </Select>
                </Field>
                <Button type="submit">Create template</Button>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Copy semantics</CardTitle>
              <CardDescription>Publishing controls catalog visibility only.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Applied templates become user-owned strategy configs.</li>
                <li>Later template edits do not rewrite already-applied user strategies.</li>
                <li>Published templates: {state.templates.filter((item) => item.status === "published").length}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Template inventory</CardTitle>
          <CardDescription>Review draft and published templates in one table.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "name", label: "Template" },
              { key: "market", label: "Market" },
              { key: "mode", label: "Mode" },
              { key: "status", label: "Status" },
              { key: "copies", label: "Copies", align: "right" },
              { key: "action", label: "Action", align: "right" },
            ]}
            rows={state.templates.map((item) => ({
              id: item.id,
              action:
                item.status === "draft" ? (
                  <FormStack action="/api/admin/templates" method="post">
                    <input name="templateId" type="hidden" value={item.id} />
                    <input name="intent" type="hidden" value="publish" />
                    <Button type="submit">Publish {item.name}</Button>
                  </FormStack>
                ) : (
                  <Chip tone="success">Published</Chip>
                ),
              copies: String(item.copies),
              market: item.market,
              mode: item.mode,
              name: item.name,
              status: <Chip tone={item.status === "published" ? "success" : "warning"}>{item.status}</Chip>,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
