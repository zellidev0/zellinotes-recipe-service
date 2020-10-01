use actix_web::{Either, HttpRequest, HttpResponse, Responder, web};
use actix_web::dev::HttpResponseBuilder;
use actix_web::web::{Json, Query};
use bson::oid::ObjectId;

use crate::dao;
use crate::dao::Dao;
use crate::model::recipe::Recipe;
use crate::pagination::Pagination;

type RoutesError = String;

pub struct RecipeRoutes {}

impl RecipeRoutes {
    pub async fn update_one_recipe(req: HttpRequest, database: web::Data<Dao>, recipe: Json<Recipe>) -> impl Responder {
        let id = match extract_id_from_req(req) {
            Ok(id) => id,
            Err(err) => return err
        };

        match database.update_one_recipe(id, recipe.into_inner()).await {
            Some(updated) => match updated {
                Some(_) => HttpResponse::Ok(),
                None => HttpResponse::NotFound(),
            },
            None => HttpResponse::InternalServerError()
        }
    }

    pub async fn add_one_recipe(database: web::Data<Dao>, recipe: Json<Recipe>) -> Either<impl Responder, impl Responder> {
        match database.add_one_recipe(recipe.into_inner()).await {
            Some(bson) => Either::A(HttpResponse::Ok().json(bson)),
            None => Either::B(HttpResponse::InternalServerError())
        }
    }

    pub async fn delete_one_recipe(req: HttpRequest, database: web::Data<Dao>) -> impl Responder {
        let id = match extract_id_from_req(req) {
            Ok(id) => id,
            Err(bad_request) => return bad_request
        };

        match database.delete_one_recipe(id).await {
            Some(recipe_option) => match recipe_option {
                Some(_) => HttpResponse::Ok(),
                None => HttpResponse::NotFound()
            }
            None => HttpResponse::InternalServerError()
        }
    }

    pub async fn add_many_recipes(database: web::Data<Dao>, recipes: Json<Vec<Recipe>>) -> Either<impl Responder, impl Responder> {
        match database.add_many_recipes(recipes.into_inner()).await {
            Some(bson) => Either::A(HttpResponse::Ok().json(bson)),
            None => Either::B(HttpResponse::InternalServerError())
        }
    }

    pub async fn get_one_recipe(req: HttpRequest, database: web::Data<Dao>) -> Either<impl Responder, impl Responder> {
        let id = match extract_id_from_req(req) {
            Ok(id) => id,
            Err(bad_request) => return Either::A(bad_request)
        };

        match database.get_one_recipe(id).await {
            Some(recipe_option) => match recipe_option {
                Some(recipe) => Either::B(HttpResponse::Ok().json(recipe)),
                None => Either::A(HttpResponse::NotFound())
            }
            None => Either::A(HttpResponse::InternalServerError())
        }
    }

    pub async fn get_many_recipes(params: Query<Pagination>, database: web::Data<Dao>) -> Either<impl Responder, impl Responder> {
        return if params.is_fully_set() {
            info!("get recipes with pagination: {:?}", params);
            match database.get_many_recipes(Some(params.0)).await {
                Some(recipes) => Either::A(HttpResponse::Ok().json(recipes)),
                None => Either::B(HttpResponse::InternalServerError())
            }
        } else if params.is_fully_empty() {
            info!("get recipes no pagination");
            match database.get_many_recipes(None).await {
                Some(recipes) => Either::A(HttpResponse::Ok().json(recipes)),
                None => Either::B(HttpResponse::InternalServerError())
            }
        } else {
            error!("get recipes with wrong pagination: {:?}", params);
            Either::B(HttpResponse::BadRequest())
        };
    }
}


fn extract_id_from_req(req: HttpRequest) -> Result<String, HttpResponseBuilder> {
    match req.match_info().get("id") {
        Some(id) => {
            if is_valid_object_id(id) {
                Ok(id.to_string())
            } else {
                error!("Error provided id is no Object id");
                return Err(HttpResponse::BadRequest());
            }
        }
        None => {
            error!("Error getting id param from HTTP request={:#?}", req);
            return Err(HttpResponse::BadRequest());
        }
    }
}

fn is_valid_object_id(id: &str) -> bool {
    match ObjectId::with_string(&id) {
        Ok(_) => true,
        Err(_) => false
    }
}


#[cfg(test)]
mod tests {
    use actix_web::{App, test, web};
    use bson::Bson;
    use serial_test::serial;

    use crate::{Dao, init_logger};
    use crate::dao::dao_tests::{before, cleanup_after};
    use crate::dao::get_one_recipe;
    use crate::recipe_routes::RecipeRoutes;

    fn create_many_recipes() -> Bson {
        let vector = vec!(create_one_recipe_no_ingredients(),
                          create_one_recipe_with_ingredients(),
                          create_one_recipe_with_ingredients()
        );
        return Bson::Array(vector);
    }

    fn create_one_recipe_no_ingredients() -> Bson {
        bson!(
        {
            "cookingTimeInMinutes": 12,
            "created": "2020-09-11T12:21:21+00:00",
            "lastModified": "2020-09-11T12:21:21+00:00",
            "ingredients": [],
            "version": 1,
            "difficulty": "Easy",
            "description": "",
            "title": "Spaghetti",
            "tags": [],
            "image": null,
            "instructions": [],
            "defaultServings": 2
        })
    }

    fn create_one_recipe_with_ingredients() -> Bson {
        bson!(
        {
            "cookingTimeInMinutes": 12,
            "created": "2020-09-11T12:21:21+00:00",
            "lastModified": "2020-09-11T12:21:21+00:00",
            "ingredients": [
                {
                    "id": "0",
                    "amount": 200,
                    "title" : "Wheat",
                    "measurementUnit": "Kilogramm"
                },
                {
                    "id": "1",
                    "amount": 3000,
                    "title" : "Milk",
                    "measurementUnit": "Milliliter"
                }
            ],
            "version": 1,
            "difficulty": "Easy",
            "description": "",
            "title": "Spaghetti",
            "tags": [],
            "image": null,
            "instructions": [],
            "defaultServings": 2
        })
    }

    #[actix_rt::test]
    #[serial]
    async fn test_add_single_recipe() {
        let dao = before().await;

        let mut app = test::init_service(App::new()
            .data(dao.clone())
            .route("/addOneRecipe", web::post().to(RecipeRoutes::add_one_recipe))).await;

        let req = test::TestRequest::post().uri("/addOneRecipe").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_client_error());

        let payload = create_many_recipes();
        let req = test::TestRequest::post()
            .set_json(&payload).uri("/addOneRecipe").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_client_error(), "{}", resp.status());

        let payload = create_one_recipe_no_ingredients();
        let req = test::TestRequest::post()
            .set_json(&payload).uri("/addOneRecipe").to_request();
        let resp = test::call_service(&mut app, req).await;
        println!("{:#?}", resp);
        assert!(resp.status().is_success(), "{}", resp.status());

        let payload = create_one_recipe_with_ingredients();
        let req = test::TestRequest::post()
            .set_json(&payload).uri("/addOneRecipe").to_request();
        let resp = test::call_service(&mut app, req).await;
        println!("{:#?}", resp);
        assert!(resp.status().is_success(), "{}", resp.status());

        cleanup_after(dao).await;
    }

    #[actix_rt::test]
    #[serial]
    async fn test_add_many_recipes() {
        let dao = before().await;

        let mut app = test::init_service(App::new()
            .data(dao.clone())
            .route("/addManyRecipes", web::post().to(RecipeRoutes::add_many_recipes))).await;

        let req = test::TestRequest::post().uri("/addManyRecipes").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_client_error());

        let payload = create_one_recipe_no_ingredients();
        let req = test::TestRequest::post()
            .set_json(&payload)
            .uri("/addManyRecipes").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_client_error());

        let payload = create_many_recipes();
        let req = test::TestRequest::post()
            .set_json(&payload).uri("/addManyRecipes").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success(), "{}", resp.status());

        cleanup_after(dao).await;
    }


    #[actix_rt::test]
    #[serial]
    async fn test_get_many_recipes() {
        let dao = before().await;

        let mut app = test::init_service(App::new()
            .data(dao.clone())
            .route("/recipes", web::get().to(RecipeRoutes::get_many_recipes))
            .route("/addManyRecipes", web::post().to(RecipeRoutes::add_many_recipes))).await;

        let req = test::TestRequest::get().uri("/recipes").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success(), "{}", resp.status());


        let payload = create_many_recipes();
        let payload = payload.as_array().unwrap().clone();
        let payload: Vec<Bson> = (0..50).into_iter().map(|_| payload.get(0).unwrap().clone()).collect();
        let payload = Bson::Array(payload);

        let req = test::TestRequest::post()
            .set_json(&payload).uri("/addManyRecipes").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success(), "{}", resp.status());


        let req = test::TestRequest::get().uri("/recipes").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success(), "{}", resp.status());

        cleanup_after(dao).await;
    }

    #[actix_rt::test]
    #[serial]
    async fn test_get_one_recipe() {
        let dao = before().await;

        let mut app = test::init_service(App::new()
            .data(dao.clone())
            .route("/recipes/{id}", web::get().to(RecipeRoutes::get_one_recipe))
            .route("/recipes/{id}", web::post().to(RecipeRoutes::add_one_recipe))).await;

        let req = test::TestRequest::get().uri("/recipes/hello").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_client_error(), "{}", resp.status());

        let payload = create_one_recipe_no_ingredients().as_document().unwrap().clone();

        let req = test::TestRequest::post()
            .set_json(&payload).uri("/recipes/new").to_request();

        let resp = test::call_service(&mut app, req).await;
        println!("{:#?}", resp);
        assert!(resp.status().is_success(), "{}", resp.status());

        let req = test::TestRequest::get().uri("/recipes/hello").to_request();
        let mut resp = test::call_service(&mut app, req).await;
        // let body = resp.response_mut().take_body().try_fold(|e| e);
        // let x = body.as_ref().unwrap().to_owned();
        // let x1 = std::str::from_utf8(x).unwrap();
        // println!("{:#?}", x);

        assert!(resp.status().is_client_error(), "{}", resp.status());


        cleanup_after(dao).await;
    }

    #[actix_rt::test]
    async fn test_update_one_recipe() {
        let dao = before().await;

        let mut app = test::init_service(App::new()
            .data(dao.clone())
            .route("/recipes/{id}", web::get().to(RecipeRoutes::get_one_recipe))
            .route("/recipes/{id}", web::post().to(RecipeRoutes::add_one_recipe))
            .route("/recipes/{id}", web::put().to(RecipeRoutes::update_one_recipe))).await;

        let mut payload = create_one_recipe_no_ingredients().as_document().unwrap().clone();

        let req = test::TestRequest::post()
            .set_json(&payload).uri("/recipes/new").to_request();

        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success(), "{}", resp.status());

        //     todo get body from resp and extract id.

        payload.insert("difficulty", "Medium");
        let id = "5f7333360051027600b01a36".to_string();
        let url = format!("/recipes/{}", id);

        let req = test::TestRequest::put().set_json(&payload).uri(&url).to_request();

        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success(), "{}", resp.status());
        // todo check if recipe was updated


        cleanup_after(dao).await;
    }
}
