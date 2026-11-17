# Web app → Axum: what replaces each Supabase call

All routes are under `/api/v1`, send `Authorization: Bearer <supabase access token>`, and answer
`{success, data}` or `{success:false, error:{code,message}}`. Role: `admin` (and `super_admin`).

| Web (DataContext / AuthContext)                         | Axum |
|---|---|
| supabase.auth.signInWithPassword / signUp / signOut / refresh | `POST /auth/login` · `POST /auth/register` (login + FOC in one call) · `POST /auth/logout` · `POST /auth/refresh` · `POST /auth/forgot-password`. Response `{session:{access_token,refresh_token,expires_in,expires_at}, user, me}`. Keep the access token in memory/storage and send it as `Bearer`; refresh before `expires_at`. |
| fetchProfile + fetchFoc                                  | `GET /auth/me` (ids, role, foc_status) + `GET /foc` |
| checkUserExists / checkUserExistsAndRole (login)         | remove: sign in with Supabase, then `GET /auth/me`; refuse non-admin in the UI |
| rpc create_foc_and_user (register)                       | `POST /auth/register` (preferred) or `POST /auth/register-foc` for an already signed-up user → FOC starts `pending` |
| fetchProfiles / createProfile / updateProfile            | `GET /members?search=&status=&cell_group_id=` · `POST /members` · `PATCH /members/{id}` |
| approveMember / setPendingMember / rejectMember / release| `POST /members/{id}/approve` · `/pending` · `/reject` · `/release {status}` |
| deleteProfile                                            | `DELETE /members/{id}` (soft) |
| family (fetch/add/remove)                                | `GET/POST /members/{id}/family` · `DELETE /family/{relationship_id}` |
| events, RSVP, attendance, QR                             | `/events…` (earlier stages) |
| event_services (EventServicesPanel, EventReport)         | `GET/POST /events/{id}/services` `{member_id,service_role,service_domain?,notes?}` · `DELETE /event-services/{id}` |
| event_serving                                            | `GET/POST /events/{id}/serving` · `DELETE /event-serving/{id}` |
| event_income (merged into incomes)                       | `POST /incomes` with `event_id` |
| sermons / prayers / testimonies / announcements          | earlier stages |
| ministries list + selection + leaders                    | `GET /ministries` (all, with `selected`, `leader_id`) · `PUT /ministries/{id}/selected {selected}` · `PUT /ministries/{id}/leader {leader_id}` |
| createMinistry                                           | `POST /ministries {name,description}` (auto-enabled for your FOC) |
| updateMinistry / deleteMinistry                          | super_admin only: `PATCH/DELETE /ministries/{id}` |
| courses                                                  | `GET/POST /courses` · super_admin `PATCH/DELETE /courses/{id}` |
| cell groups / meetings                                   | `/cell-groups…` · `GET /cell-meetings` · `PATCH/DELETE /cell-meetings/{id}` |
| social_events                                            | `GET/POST /social-events` · `PATCH/DELETE /social-events/{id}` (type: Birthday, Anniversary, Memorial, Other) |
| bible reading plans                                      | `GET/POST /bible-plans` · `PATCH/DELETE /bible-plans/{id}` (own only) |
| currencies / categories / methods + selection + usage    | `/finance/setup`, `/finance/currencies…`; usage: `GET /finance/{currencies,categories,methods}/{id}/usage` → `{count}` |
| incomes / expenses (+ contribution_changes)              | `/incomes…`, `/expenses…`, `/finance/history/{id}`; void instead of delete |
| fetchFocs / fetchChurches / fetchProvinces               | `GET /focs` (your church) · `GET /church` · `GET /provinces` |
| updateFoc                                                | `PATCH /foc` (your own FOC only) |
| createFoc / deleteFoc                                    | not for FOC admins; platform owner: `GET /platform/focs`, `PUT /platform/focs/{id}/status` |
| fetchMemberContributions / Attendance / Activity         | `GET /members/{id}/contributions` · `/attendance` · `/activity` |
| sermon audio (storage upload/remove)                     | `POST /sermons/{id}/audio/upload-url` → PUT the file to `upload_url` → `POST /sermons/{id}/audio/confirm`; remove: `DELETE /sermons/{id}/audio` |
| fetchLogos (storage list)                                | `GET /logos` |
| dashboard counts                                         | `GET /dashboard` |

## Still on Supabase (behind Axum)
Postgres, Auth (users/passwords/tokens) and Storage (files) are Supabase's engines, but only Axum talks to them
for the apps: logins, uploads and deletes all go through Axum, which holds the keys.
Realtime channels still come straight from Supabase for now (to be replaced by a signal-only channel).

## Sermon text
Titles, preachers and scripture are collapsed to single clean lines; summary and body keep paragraphs but lose stray
spaces, tabs, control/zero-width characters and extra blank lines. The server does this on every create/update.
