// The ops app's API on the web server, under /api/ops: every route is
// for staff. The browser sends the session cookie itself.

/**
 * The signed-in person's staff row, or why not: "signed-out" when
 * nobody is signed in, "not-staff" when they are not staff.
 * @returns {Promise<{user_id: string, inserted_at: string} | "signed-out" | "not-staff">}
 */
export async function opsCurrentStaffGet() {
  const response = await fetch("/api/ops/current-staff");
  if (response.status === 401) return "signed-out";
  if (response.status === 403) return "not-staff";
  if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
  return response.json();
}
