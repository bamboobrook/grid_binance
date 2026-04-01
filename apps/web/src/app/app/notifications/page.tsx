const notificationTypes = [
  "Deposit confirmed",
  "Membership expiring",
  "Runtime failure",
];

export default function NotificationsPage() {
  return (
    <main>
      <h1>Notifications</h1>
      <p>
        Review Telegram delivery status and the in-app inbox for deposit updates,
        membership reminders, and runtime alerts.
      </p>
      <ul>
        {notificationTypes.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </main>
  );
}
