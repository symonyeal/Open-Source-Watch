//! Persisted absence must not become an ordinary identity, coordinate or time.
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, RecordBatch};
use arrow_schema::{Field, Schema};
use osw::schema::{Event, Obs, Raw, Table};

fn without<T: Table>(name: &str) -> RecordBatch {
    let b = T::batch(&[]);
    let fields: Vec<_> = b
        .schema()
        .fields()
        .iter()
        .filter(|f| f.name() != name)
        .cloned()
        .collect();
    let columns = b
        .schema()
        .fields()
        .iter()
        .zip(b.columns())
        .filter(|(f, _)| f.name() != name)
        .map(|(_, a)| a.clone())
        .collect();
    RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap()
}

#[test]
#[should_panic(expected = "raw_report: required column id is absent")]
fn missing_identity_is_rejected_even_in_empty_partition() {
    Raw::rows(&without::<Raw>("id"));
}

#[test]
#[should_panic(expected = "event: required column t_start is absent")]
fn missing_time_is_rejected() {
    Event::rows(&without::<Event>("t_start"));
}

#[test]
#[should_panic(expected = "event: required column observation_ids is absent")]
fn missing_membership_is_rejected() {
    Event::rows(&without::<Event>("observation_ids"));
}

#[test]
#[should_panic(expected = "event: required column lat contains nulls")]
fn null_coordinate_is_rejected() {
    let b = Event::batch(&[Event::default()]);
    let fields: Vec<_> = b
        .schema()
        .fields()
        .iter()
        .map(|f| {
            if f.name() == "lat" {
                Arc::new(Field::new("lat", f.data_type().clone(), true))
            } else {
                f.clone()
            }
        })
        .collect();
    let columns: Vec<ArrayRef> = b
        .schema()
        .fields()
        .iter()
        .zip(b.columns())
        .map(|(f, a)| {
            if f.name() == "lat" {
                Arc::new(Float64Array::from(vec![None::<f64>])) as ArrayRef
            } else {
                a.clone()
            }
        })
        .collect();
    Event::rows(&RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap());
}

#[test]
#[should_panic(expected = "non-finite measurement")]
fn nonfinite_coordinate_cannot_be_written() {
    Event::batch(&[Event {
        lat: f64::NAN,
        ..Event::default()
    }]);
}

#[test]
#[should_panic(expected = "lon: outside -180..180")]
fn invalid_coordinate_cannot_be_read() {
    let b = Event::batch(&[Event::default()]);
    let mut columns = b.columns().to_vec();
    let i = b.schema().index_of("lon").unwrap();
    columns[i] = Arc::new(Float64Array::from(vec![181.0]));
    Event::rows(&RecordBatch::try_new(b.schema(), columns).unwrap());
}

#[test]
fn optional_annotation_and_historical_list_have_explicit_absence() {
    let r = Obs::default();
    let b = Obs::batch(std::slice::from_ref(&r));
    let keep: Vec<usize> = b
        .schema()
        .fields()
        .iter()
        .enumerate()
        .filter(|(_, f)| !["admin1", "weapon"].contains(&f.name().as_str()))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(Obs::rows(&b.project(&keep).unwrap()), vec![r]);
    assert!(!Raw::schema().field_with_name("id").unwrap().is_nullable());
    assert!(
        Obs::schema()
            .field_with_name("admin1")
            .unwrap()
            .is_nullable()
    );
}
