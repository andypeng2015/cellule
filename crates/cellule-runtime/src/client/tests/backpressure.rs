use super::*;

fn admission_transport(
    requests: usize,
    bytes: usize,
    wait: std::time::Duration,
) -> super::super::backpressure::BackpressureTransport {
    let target = CellTarget::new(
        TenantId::from_bytes([1; 16]),
        ApplicationId::from_bytes([2; 16]),
        NAMESPACE,
        b"admission",
    )
    .unwrap();
    super::super::backpressure::BackpressureTransport::new(
        Arc::new(StreamTransport {
            description: CellDescription {
                cell: target.cell_id(),
                incarnation: IncarnationId::from_bytes([3; 16]),
                code: RETAINED_CODE,
                schema: 1,
            },
            sequence: Arc::default(),
            fenced: Arc::default(),
            query_started: None,
            query_release: None,
        }),
        requests,
        bytes,
        wait,
    )
    .unwrap()
}

#[tokio::test]
async fn admission_backpressure_retries_capacity_but_not_fencing() {
    let transport = admission_transport(2, 8_192, std::time::Duration::from_secs(1));
    let mut attempts = 0;
    let value = transport
        .invoke(ADMISSION_CELL, 32, Some(1), || {
            attempts += 1;
            std::future::ready(if attempts < 3 {
                Err(Error::Capacity("owner mailbox"))
            } else {
                Ok(42)
            })
        })
        .await
        .unwrap();
    assert_eq!((value, attempts), (42, 3));
    let mut attempts = 0;
    let result = transport
        .invoke(ADMISSION_CELL, 32, Some(1), || {
            attempts += 1;
            std::future::ready(Err::<(), _>(Error::Fenced))
        })
        .await;
    assert!(matches!(result, Err(Error::Fenced)) && attempts == 1);
}

#[tokio::test]
async fn admission_backpressure_releases_shared_limits_after_cancellation() {
    for (requests, bytes, resource) in [
        (1, 8_192, "client admission requests"),
        (2, 2_048, "client admission bytes"),
    ] {
        let transport = admission_transport(requests, bytes, std::time::Duration::from_secs(1));
        let clone = transport.clone();
        let entered = Notify::new();
        let mut held = Box::pin(transport.invoke(ADMISSION_CELL, 0, Some(1), || {
            entered.notify_one();
            std::future::pending::<crate::Result<()>>()
        }));
        tokio::select! {
            result = &mut held => panic!("unexpected completion: {result:?}"),
            () = entered.notified() => {}
        }
        let result = clone
            .invoke(ADMISSION_CELL, 0, Some(1), || std::future::ready(Ok(())))
            .await;
        assert!(matches!(result, Err(Error::Capacity(found)) if found == resource));
        drop(held);
        clone
            .invoke(ADMISSION_CELL, 0, Some(1), || std::future::ready(Ok(())))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn admission_backpressure_expires_without_canceling_accepted_work() {
    let transport = admission_transport(1, 8_192, std::time::Duration::from_millis(25));
    let mut attempts = 0;
    let result = transport
        .invoke(ADMISSION_CELL, 0, Some(1), || {
            attempts += 1;
            std::future::ready(Err::<(), _>(Error::Capacity("owner mailbox")))
        })
        .await;
    assert!(matches!(result, Err(Error::Capacity("owner mailbox"))) && attempts <= 2);
    let result = transport
        .invoke(ADMISSION_CELL, 0, Some(1), || async {
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            Ok(42)
        })
        .await
        .unwrap();
    assert_eq!(result, 42);
}

#[tokio::test]
async fn admission_backpressure_is_fifo_per_cell_and_independent_across_cells() {
    let transport = admission_transport(4, 16_384, std::time::Duration::from_secs(1));
    let mut held = Box::pin(transport.invoke(
        ADMISSION_CELL,
        0,
        Some(crate::cell::actor::CELL_BYTES),
        std::future::pending::<crate::Result<()>>,
    ));
    assert!(futures_util::poll!(&mut held).is_pending());
    let order = AtomicUsize::new(0);
    let mut queued = Box::pin(transport.invoke(
        ADMISSION_CELL,
        0,
        Some(crate::cell::actor::CELL_BYTES),
        || std::future::ready(Ok(order.fetch_add(1, Ordering::SeqCst))),
    ));
    assert!(futures_util::poll!(&mut queued).is_pending());
    transport
        .invoke(crate::CellId::from_bytes([2; 32]), 0, Some(1), || {
            std::future::ready(Ok(()))
        })
        .await
        .unwrap();
    drop(held);
    let arrival = transport.invoke(
        ADMISSION_CELL,
        0,
        Some(crate::cell::actor::CELL_BYTES),
        || std::future::ready(Ok(order.fetch_add(1, Ordering::SeqCst))),
    );
    // Poll the new arrival first: the already queued call must still run first.
    let (arrival, queued) = tokio::join!(biased; arrival, queued);
    assert_eq!((queued.unwrap(), arrival.unwrap()), (0, 1));
}

#[tokio::test]
async fn admission_backpressure_overlaps_calls_without_overtaking_large_waiters() {
    let transport = admission_transport(4, 16_384, std::time::Duration::from_secs(1));
    let half = crate::cell::actor::CELL_BYTES / 2;
    let mut held = Box::pin(transport.invoke(ADMISSION_CELL, 0, Some(half), || {
        std::future::pending::<crate::Result<()>>()
    }));
    assert!(futures_util::poll!(&mut held).is_pending());
    transport
        .invoke(ADMISSION_CELL, 0, Some(half), || std::future::ready(Ok(())))
        .await
        .unwrap();
    let mut large = Box::pin(transport.invoke(ADMISSION_CELL, 0, Some(half + 1), || {
        std::future::ready(Ok(()))
    }));
    assert!(futures_util::poll!(&mut large).is_pending());
    let mut small =
        Box::pin(transport.invoke(ADMISSION_CELL, 0, Some(1), || std::future::ready(Ok(()))));
    assert!(futures_util::poll!(&mut small).is_pending());
    // Canceling the older large waiter releases its claim on remaining bytes.
    drop(large);
    small.await.unwrap();
}

#[tokio::test]
async fn describe_admission_does_not_wait_for_the_owner_mailbox() {
    let transport = admission_transport(2, 8_192, std::time::Duration::from_millis(25));
    let target = CellTarget::new(
        TenantId::from_bytes([1; 16]),
        ApplicationId::from_bytes([2; 16]),
        NAMESPACE,
        b"admission",
    )
    .unwrap();
    let mut held = Box::pin(transport.invoke(
        target.cell_id(),
        0,
        Some(crate::cell::actor::CELL_BYTES),
        std::future::pending::<crate::Result<()>>,
    ));
    assert!(futures_util::poll!(&mut held).is_pending());
    assert_eq!(
        transport.describe(target.clone()).await.unwrap().cell,
        target.cell_id()
    );
}
