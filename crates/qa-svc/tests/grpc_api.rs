mod common;
#[path = "common/pulsar.rs"]
mod test_pulsar;
#[path = "common/redis.rs"]
mod test_redis;

use anyhow::{Context, Result, ensure};
use common::TestDatabase;
use pb::{
    AddAnswerRequest, AddQuestionRequest, AnswerDetailRequest, AnswerEntity, AnswerListRequest,
    DeleteAnswerRequest, DeleteQuestionRequest, LatestQuestionRequest, QuestionDetailRequest,
    UpdateAnswerRequest, UserLoginRequest, UserLogoutRequest, UserRegisterRequest,
    VerifyTokenRequest,
    qa_service_client::QaServiceClient,
    qa_service_server::{QaService, QaServiceServer},
};
use qa_svc::{AppState, QaServiceImpl};
use qa_sys_core::RedisPool;
use test_pulsar::pulsar_client;
use test_redis::redis_pool;
use tonic::transport::{Server, server::TcpIncoming};

fn clear_redis_database(pool: &RedisPool) -> Result<()> {
    match pool {
        RedisPool::Single(pool) => {
            let mut connection = pool.get()?;
            redis::cmd("FLUSHDB").query::<()>(&mut *connection)?;
        }
        RedisPool::Cluster(pool) => {
            let mut connection = pool.get()?;
            redis::cmd("FLUSHDB").query::<()>(&mut *connection)?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_should_register_new_user_and_reject_duplicate() -> Result<()> {
    let database = TestDatabase::create().await?;
    let redis = redis_pool()?;
    clear_redis_database(&redis)?;
    let result = async {
        let service = QaServiceImpl::new(AppState {
            pgsql_pool: database.pool.clone(),
            pulsar_client: pulsar_client().await?,
            redis_pool: redis.clone(),
        });
        let request = UserRegisterRequest {
            username: "alice".to_string(),
            password: "secret-password".to_string(),
            email: "alice@example.com".to_string(),
            phone: "12345678".to_string(),
        };

        let registered = service
            .user_register(tonic::Request::new(request.clone()))
            .await?;
        ensure!(registered.into_inner().state == 1);

        let duplicate = service.user_register(tonic::Request::new(request)).await;
        ensure!(matches!(
            duplicate,
            Err(status) if status.code() == tonic::Code::AlreadyExists
        ));
        Ok(())
    }
    .await;
    clear_redis_database(&redis)?;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_login_verify_and_logout_user() -> Result<()> {
    let database = TestDatabase::create().await?;
    let redis = redis_pool()?;
    clear_redis_database(&redis)?;
    let result = async {
        let service = QaServiceImpl::new(AppState {
            pgsql_pool: database.pool.clone(),
            pulsar_client: pulsar_client().await?,
            redis_pool: redis.clone(),
        });
        service
            .user_register(tonic::Request::new(UserRegisterRequest {
                username: "alice".to_string(),
                password: "secret-password".to_string(),
                email: "alice@example.com".to_string(),
                phone: "12345678".to_string(),
            }))
            .await?;

        let login = service
            .user_login(tonic::Request::new(UserLoginRequest {
                username: "alice".to_string(),
                password: "secret-password".to_string(),
            }))
            .await?
            .into_inner();
        ensure!(!login.token.is_empty());

        let verified = service
            .verify_token(tonic::Request::new(VerifyTokenRequest {
                token: login.token.clone(),
                request_id: "verify-before-logout".to_string(),
            }))
            .await?
            .into_inner();
        ensure!(verified.state == 1);
        ensure!(verified.username == "alice");

        let logout = service
            .user_logout(tonic::Request::new(UserLogoutRequest {
                token: login.token.clone(),
            }))
            .await?
            .into_inner();
        ensure!(logout.state == 1);

        let after_logout = service
            .verify_token(tonic::Request::new(VerifyTokenRequest {
                token: login.token,
                request_id: "verify-after-logout".to_string(),
            }))
            .await?
            .into_inner();
        ensure!(after_logout.state == 0);
        ensure!(after_logout.username.is_empty());
        Ok(())
    }
    .await;
    clear_redis_database(&redis)?;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_manage_question_and_answer_lifecycle() -> Result<()> {
    let database = TestDatabase::create().await?;
    let redis = redis_pool()?;
    clear_redis_database(&redis)?;
    let result = async {
        let service = QaServiceImpl::new(AppState {
            pgsql_pool: database.pool.clone(),
            pulsar_client: pulsar_client().await?,
            redis_pool: redis.clone(),
        });

        let question_id = service
            .add_question(tonic::Request::new(AddQuestionRequest {
                title: "How does Rust ownership work?".to_string(),
                content: "Please explain ownership.".to_string(),
                create_by: "alice".to_string(),
            }))
            .await?
            .into_inner()
            .id;
        ensure!(question_id > 0);

        let detail = service
            .question_detail(tonic::Request::new(QuestionDetailRequest {
                id: question_id,
                username: "alice".to_string(),
            }))
            .await?
            .into_inner()
            .question;
        ensure!(detail.is_some());
        let detail = detail.context("question detail must be present")?;
        ensure!(detail.title == "How does Rust ownership work?");
        ensure!(detail.read_count == 1);

        let latest = service
            .latest_question(tonic::Request::new(LatestQuestionRequest {
                last_id: 0,
                limit: 10,
            }))
            .await?
            .into_inner();
        ensure!(latest.questions.len() == 1);
        ensure!(latest.questions[0].id == question_id);
        ensure!(latest.is_end);

        let answer_id = service
            .add_answer(tonic::Request::new(AddAnswerRequest {
                answer: Some(AnswerEntity {
                    question_id,
                    content: "A value has one owner.".to_string(),
                    create_by: "bob".to_string(),
                    ..Default::default()
                }),
            }))
            .await?
            .into_inner()
            .id;
        ensure!(answer_id > 0);

        let answer = service
            .answer_detail(tonic::Request::new(AnswerDetailRequest {
                id: answer_id,
                username: "alice".to_string(),
            }))
            .await?
            .into_inner()
            .answer
            .context("answer detail must be present")?;
        ensure!(answer.content == "A value has one owner.");
        ensure!(!answer.has_aggreed);

        let answers = service
            .answer_list(tonic::Request::new(AnswerListRequest {
                question_id,
                page: 1,
                limit: 10,
                username: "alice".to_string(),
            }))
            .await?
            .into_inner();
        ensure!(answers.total == 1);
        ensure!(answers.answers.len() == 1);
        ensure!(answers.answers[0].id == answer_id);

        service
            .update_answer(tonic::Request::new(UpdateAnswerRequest {
                id: answer_id,
                content: "Ownership also enables borrowing.".to_string(),
                username: "bob".to_string(),
            }))
            .await?;
        let updated = service
            .answer_detail(tonic::Request::new(AnswerDetailRequest {
                id: answer_id,
                username: "bob".to_string(),
            }))
            .await?
            .into_inner()
            .answer
            .context("updated answer detail must be present")?;
        ensure!(updated.content == "Ownership also enables borrowing.");

        service
            .delete_answer(tonic::Request::new(DeleteAnswerRequest {
                id: answer_id,
                username: "bob".to_string(),
            }))
            .await?;
        ensure!(
            service
                .answer_detail(tonic::Request::new(AnswerDetailRequest {
                    id: answer_id,
                    username: "bob".to_string(),
                }))
                .await
                .is_err()
        );

        service
            .delete_question(tonic::Request::new(DeleteQuestionRequest {
                id: question_id,
                username: "alice".to_string(),
            }))
            .await?;
        ensure!(
            service
                .question_detail(tonic::Request::new(QuestionDetailRequest {
                    id: question_id,
                    username: "alice".to_string(),
                }))
                .await
                .is_err()
        );
        Ok(())
    }
    .await;
    clear_redis_database(&redis)?;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_serve_authentication_over_grpc_transport() -> Result<()> {
    let database = TestDatabase::create().await?;
    let redis = redis_pool()?;
    clear_redis_database(&redis)?;
    let incoming = TcpIncoming::bind("127.0.0.1:0".parse()?)?;
    let address = incoming.local_addr()?;
    let service = QaServiceImpl::new(AppState {
        pgsql_pool: database.pool.clone(),
        pulsar_client: pulsar_client().await?,
        redis_pool: redis.clone(),
    });
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(QaServiceServer::new(service))
            .serve_with_incoming_shutdown(incoming, async {
                let _result = shutdown_receiver.await;
            })
            .await
    });

    let result = async {
        let mut client = QaServiceClient::connect(format!("http://{address}")).await?;
        let register = client
            .user_register(UserRegisterRequest {
                username: "transport-user".to_string(),
                password: "secret-password".to_string(),
                email: "transport@example.com".to_string(),
                phone: "12345678".to_string(),
            })
            .await?
            .into_inner();
        ensure!(register.state == 1);

        let login = client
            .user_login(UserLoginRequest {
                username: "transport-user".to_string(),
                password: "secret-password".to_string(),
            })
            .await?
            .into_inner();
        ensure!(!login.token.is_empty());

        let verified = client
            .verify_token(VerifyTokenRequest {
                token: login.token,
                request_id: "transport-verification".to_string(),
            })
            .await?
            .into_inner();
        ensure!(verified.state == 1);
        ensure!(verified.username == "transport-user");
        Ok(())
    }
    .await;

    let _shutdown_result = shutdown_sender.send(());
    let server_result = server.await.context("gRPC server task must not panic")?;
    server_result.context("gRPC server must shut down cleanly")?;
    clear_redis_database(&redis)?;
    database.cleanup().await?;
    result
}
