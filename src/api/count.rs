use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::content::RawJson;
use rocket::serde::{json::Json, Serialize};
use rocket::State;

use std::convert::Infallible;
use std::net::{self, IpAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::dev::User;

#[derive(Serialize, Default)]
#[serde(crate = "rocket::serde")]
pub struct IPList {
	pub ips: Vec<net::IpAddr>,
}

pub struct UserCount {
	count: AtomicUsize,
	ips: Mutex<IPList>,
}

impl Default for UserCount {
	fn default() -> Self {
		UserCount {
			count: AtomicUsize::new(0),
			ips: Mutex::new(IPList::default()),
		}
	}
}

impl UserCount {
	pub fn count(&self) -> usize {
		self.count.load(Ordering::Relaxed)
	}

	pub fn ips(&self) -> Option<Vec<net::IpAddr>> {
		match self.ips.lock() {
			Ok(v) => Some(v.ips.to_vec()),
			Err(_) => None,
		}
	}
}

#[get("/count")]
pub fn count(user_count: &State<UserCount>) -> RawJson<String> {
	RawJson(format!(
		"{{\"user_count\": {}}}",
		user_count.count.load(Ordering::Relaxed)
	))
}

#[get("/count/ips")]
pub fn ips(_user: User, user_count: &State<UserCount>) -> Json<IPList> {
	match user_count.ips.lock() {
		Ok(v) => Json(IPList { ips: v.ips.clone() }),
		_ => Json(IPList { ips: vec![] }),
	}
}

pub struct PublicIp(pub Option<IpAddr>);
#[rocket::async_trait]
impl<'r> FromRequest<'r> for PublicIp {
	type Error = Infallible;

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let do_connecting = request.headers().get_one("do-connecting-ip");
		match do_connecting {
			Some(ip) => match ip.parse::<IpAddr>(){
				Ok(parsed) => Outcome::Success(PublicIp(Some(parsed))),
				Err(_) => Outcome::Success(PublicIp(None)),
			}
			None => Outcome::Success(PublicIp(None)),
		}
	}
}

#[post("/count")]
pub fn inc_count(user_count: &State<UserCount>, ip: IpAddr, pub_ip: PublicIp) {
	let ip = match pub_ip.0 {
		Some(public) => public,
		None => ip,
	};

	match user_count.ips.lock() {
		Ok(mut v) => {
			if v.ips.contains(&ip) {
				println!("{} already said hi", ip);
				// can add ips more than once in debug
				if cfg!(debug_assertions) {
					user_count.count.fetch_add(1, Ordering::SeqCst);
					v.ips.push(ip);
				}
			} else {
				user_count.count.fetch_add(1, Ordering::SeqCst);
				v.ips.push(ip);
				println!("new hello from {:?}", ip);
			}
		}
		Err(e) => println!("error getting ips: {}", e),
	}
}

#[post("/count/remove/<index>")]
pub fn remove_ip(_user: User, user_count: &State<UserCount>, index: usize) {
	match user_count.ips.lock() {
		Ok(mut v) => {
			if index < v.ips.len() {
				v.ips.remove(index);
			}
		}
		Err(e) => println!("error getting ips: {}", e),
	}
}

pub fn routes() -> Vec<rocket::Route> {
	routes![count, inc_count, remove_ip, ips]
}
