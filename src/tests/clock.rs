use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
struct DelayedRegistration {
    clock: AtomicU64,
    completed: u64,
    wait_before_commit: bool,
    committed: AtomicU64,
}
impl RegistrationStore for DelayedRegistration {
    async fn reserve(
        &self,
        _: &ScopedIdentity,
        _: &str,
        _: u64,
        _: &str,
        valid_until: u64,
    ) -> Result<(), Error> {
        assert_eq!(valid_until, 200);
        yield_once().await;
        if self.wait_before_commit {
            self.clock.store(self.completed, Ordering::SeqCst);
        }
        if self.clock.load(Ordering::SeqCst) >= valid_until {
            return Err(Error::Reservation);
        }
        self.committed.fetch_add(1, Ordering::SeqCst);
        yield_once().await;
        self.clock.store(self.completed, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn registration_deadline_is_checked_inside_commit_and_after_the_wait() {
    let identity = execute(&Dummy::new(RESPONSE), operation())
        .unwrap()
        .comparison(
            &IdentityKey::new([7; 32]).unwrap(),
            &identity_request(),
            &capture(),
            100,
        )
        .unwrap();
    let request = Registration {
        policy_scope: "window".into(),
        maximum: 2,
        operation: "registration".into(),
    };
    for (completed, wait_before_commit) in [(200, true), (200, false), (100 - 1, false)] {
        let store = DelayedRegistration {
            clock: AtomicU64::new(100),
            completed,
            wait_before_commit,
            committed: AtomicU64::new(0),
        };
        let result =
            block_on(identity.register(&store, &request, &|| store.clock.load(Ordering::SeqCst)));
        if wait_before_commit {
            assert_eq!(result, Err(Error::Reservation));
            assert_eq!(store.committed.load(Ordering::SeqCst), 0);
        } else {
            assert_eq!(store.committed.load(Ordering::SeqCst), 1);
            assert_eq!(
                result,
                if completed >= 200 || completed < 100 {
                    Err(Error::Rejected)
                } else {
                    Ok(())
                }
            );
        }
    }
}
struct WaitingJournal<'a> {
    clock: &'a AtomicU64,
    journal: Journal,
    advance: u64,
}
impl OperationStore for WaitingJournal<'_> {
    async fn reserve(
        &self,
        operation: &str,
        binding: &str,
        maximum: &Amount,
        valid_until: u64,
    ) -> Result<(), Error> {
        self.journal
            .reserve(operation, binding, maximum, valid_until)
            .await?;
        yield_once().await;
        self.clock.store(self.advance, Ordering::SeqCst);
        Ok(())
    }
}
struct WaitingProvider<'a> {
    clock: &'a AtomicU64,
    completed: u64,
    provider: Dummy,
}
impl Transport for WaitingProvider<'_> {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, Error> {
        let response = self.provider.send(request).await;
        yield_once().await;
        self.clock.store(self.completed, Ordering::SeqCst);
        response
    }
}
#[test]
fn provider_deadlines_cover_reservation_and_reply_without_discarding_private_reports() {
    let a = adapter();
    let clock = AtomicU64::new(100);
    let store = WaitingJournal {
        clock: &clock,
        journal: Journal::default(),
        advance: 180,
    };
    let provider = Dummy::new(RESPONSE);
    let q = a.preview(&operation(), &capture(), &tariff(), 100).unwrap();
    assert_eq!(
        block_on(a.execute(
            &provider,
            &store,
            task(),
            permit(q, Mode::Production),
            &|| clock.load(Ordering::SeqCst)
        ))
        .unwrap_err(),
        Error::Refused
    );
    assert!(provider.calls.lock().unwrap().is_empty());
    assert_eq!(store.journal.0.lock().unwrap().len(), 1);
    for completed in [99, 180, 200] {
        clock.store(100, Ordering::SeqCst);
        let store = Journal::default();
        let provider = WaitingProvider {
            clock: &clock,
            completed,
            provider: Dummy::new(RESPONSE),
        };
        let q = a.preview(&operation(), &capture(), &tariff(), 100).unwrap();
        let report = block_on(a.execute_report(
            &provider,
            &store,
            task(),
            permit(q, Mode::Production),
            &|| clock.load(Ordering::SeqCst),
        ));
        if (100..200).contains(&completed) {
            assert!(report.result.is_ok());
        } else {
            assert!(matches!(report.result, Err(Error::Uncertain)));
        }
        assert_eq!(report.response.unwrap().body.expose(), RESPONSE);
        assert_eq!(provider.provider.calls.lock().unwrap().len(), 1);
        assert_eq!(store.0.lock().unwrap().len(), 1);
    }
}

async fn yield_once() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await
}

#[test]
fn clock_rollback_after_atomic_reservation_refuses_before_provider() {
    let a = adapter();
    let clock = AtomicU64::new(100);
    let store = WaitingJournal {
        clock: &clock,
        journal: Journal::default(),
        advance: 99,
    };
    let provider = Dummy::new(RESPONSE);
    let q = a.preview(&operation(), &capture(), &tariff(), 100).unwrap();
    assert_eq!(
        block_on(a.execute(
            &provider,
            &store,
            task(),
            permit(q, Mode::Production),
            &|| clock.load(Ordering::SeqCst)
        ))
        .unwrap_err(),
        Error::Refused
    );
    assert!(provider.calls.lock().unwrap().is_empty());
    assert_eq!(store.journal.0.lock().unwrap().len(), 1);
}
