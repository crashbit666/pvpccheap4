use crate::models::Schedule;
use rocket::serde::json::Json;

#[get("/schedules")]
pub fn get_schedules() -> Json<Vec<Schedule>> {
    Json(vec![])
}

#[post("/schedules", data = "<schedule>")]
pub fn create_schedule(schedule: Json<Schedule>) -> Json<Schedule> {
    schedule
}
