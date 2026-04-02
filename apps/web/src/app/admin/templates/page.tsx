import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminTemplatesData } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ created?: string }>;
};

export default async function AdminTemplatesPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const created = typeof params.created === "string" ? params.created : "";
  const data = await getAdminTemplatesData();

  return (
    <>
      {created ? <StatusBanner description={"Created template name: " + created} title="Template created" tone="success" /> : null}
      <AppShellSection
        description="Template inventory is read from the backend strategy template catalog."
        eyebrow="Strategy templates"
        title="Template Management"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Create template</CardTitle>
              <CardDescription>Create a backend strategy template with a minimal valid payload.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/admin/templates" method="post">
                <Field label="Template name">
                  <Input name="name" placeholder="ADA Trend Rider" />
                </Field>
                <Button type="submit">Create template</Button>
              </FormStack>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Template inventory</CardTitle>
          <CardDescription>Backend template records.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "name", label: "Name" },
              { key: "symbol", label: "Symbol" },
              { key: "budget", label: "Budget" },
            ]}
            rows={data.items.map((item) => ({
              id: item.id,
              budget: item.budget,
              name: item.name,
              symbol: item.symbol,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
