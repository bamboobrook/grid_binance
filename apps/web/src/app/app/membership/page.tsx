import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getMembershipSnapshot } from "../../../lib/api/server";

export default async function MembershipPage() {
  const snapshot = await getMembershipSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="Membership state, stacked renewals, and grace transitions stay visible regardless of which user page is open."
        eyebrow="Entitlement"
        title="Membership"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Entitlement timeline</CardTitle>
              <CardDescription>Grace and renewal milestones are made explicit for user operations.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "event", label: "Event" },
                  { key: "at", label: "When" },
                  { key: "note", label: "Note" },
                ]}
                rows={snapshot.rows}
              />
            </CardBody>
          </Card>
          <DialogFrame
            description="When grace ends, all running strategies must be auto-paused and new starts blocked until membership becomes active again."
            title="Grace period enforcement"
            tone="warning"
          />
        </div>
      </AppShellSection>
    </>
  );
}
