# Maritime data and satellite validation

This platform currently records AIS samples, PortWatch aggregates and public
reports. It does not run a satellite ship detector. Its NASA VIIRS panel measures
thermal detections, which do not establish ships, cargo or an attack.

## Supplied repositories and their role

| Resource | Verified role | Use here |
| --- | --- | --- |
| [Satellite Imagery Datasets Containing Ships](https://github.com/jasonmanesis/Satellite-Imagery-Datasets-Containing-Ships) | Curated index of radar/optical datasets | Discovery index; follow each entry to its authors, labels and terms. The index's MIT licence does not license every listed dataset. |
| [HRSID, authors' repository](https://github.com/chaozhong2010/HRSID) | SAR ship detection and segmentation benchmark; 5,604 chips and 16,951 instances; includes background-only test imagery | Candidate for detection evaluation and false-positive analysis. Cropped images are not a current shipping feed. |
| [xView3-SAR paper](https://arxiv.org/abs/2206.00897), [reference implementation](https://github.com/DIUx-xView/xview3-reference) | Sentinel-1 maritime-object detection, vessel/fishing classification and length evaluation; released training, inference and scorer | Starting reference for a future SAR evaluation. Use the released scorer and scene partitions before comparing another model. No local model result is claimed. |
| [iMoonLab YOLOv13](https://github.com/iMoonLab/yolov13), [Hyper-YOLO](https://github.com/iMoonLab/Hyper-YOLO) | General object detectors with reported COCO benchmarks | Candidate architectures after a maritime baseline exists. COCO accuracy does not measure SAR accuracy or tanker classification. |
| [iMoonLab DeepHypergraph](https://github.com/iMoonLab/DeepHypergraph), [Hypergraph-DB](https://github.com/iMoonLab/Hypergraph-DB) | Hypergraph computation and storage/visualization tools | Possible research infrastructure for explicit relationships. They supply neither vessel observations nor evidence that a supply-chain relationship is causal. No dependency added. |

The public iMoonLab organization inventory was inspected on 2026-10-24: 31
repositories. Detector candidates above were checked against their own READMEs;
the remaining repositories are not claimed to be maritime systems.

## Established processing and evaluation references

[ESA's Sentinel-1 ship-detection training](https://eo4society.esa.int/event/rus-remote-training-ship-detection-with-sentinel-1-using-snap-s-1-toolbox/)
uses the SNAP Sentinel-1 Toolbox for maritime surveillance. That is a processing
reference, not evidence of this platform's detection performance. Its sensor
preprocessing must be retained when reproducing its detector; a display image
cannot be substituted silently for calibrated radar measurements.

The xView3 repository provides `reference/train_reference.ipynb`,
`reference/inference.py` and `reference/metric.py`. Its documented leaderboard
scorer invocation uses `--distance_tolerance 200`, `--shore_tolerance 2`,
`--drop_low_detect` and `--costly_dist`, with the supplied shoreline contours.
These are that benchmark's evaluation settings; they do not set AIS matching
tolerances or an operational alert threshold for this platform.

A reproducible comparison must retain the released scene partitions and scorer,
record the source commit, sensor channels, preprocessing, labels and checkpoint,
and keep validation separate from model selection. Port/country transfer needs
separately reviewed local imagery. Benchmark detection, vessel type, length and
AIS association are separate tasks. A detected or unmatched object alone cannot
establish crude cargo, vessel identity, deliberate AIS disabling or a delivery.

Implementation limits are concrete. The inspected YOLOv13 requirements pin
Torch 2.2.2 and a CPython 3.11 Linux FlashAttention wheel; this machine uses native
Windows and Python 3.14. The requirements have not been ported or tested here.
The repository is AGPL-3.0; its code, checkpoints and underlying datasets need
their own reuse review before incorporation. The xView3 reference code is
BSD-3-Clause; dataset terms are separate from that software licence.

## Operational imagery access

[Copernicus Data Space STAC](https://documentation.dataspace.copernicus.eu/APIs/STAC.html)
provides standardized collection/item discovery, including Sentinel-1 GRD.
A scene's acquisition time, footprint and product identity belong with every
derived detection. Catalogue availability does not establish a detection or
continuous coverage. A catalogue connection and a SAR detector are not installed
in this application; neither is shown as a live source.

## Implemented shipping statistics

The [World Bank chokepoints notebook](https://worldbank.github.io/alternative-data-for-crisis/notebooks/disruptions-business-trade/chokepoints-monitor.html)
uses a centered seven-day moving average. This platform follows that window
shape and requires all seven calendar observations; the notebook permits
partial edge windows. For latest day D, the displayed mean is centered at D−3
and covers D−6 through D. Missing observations make it unavailable. The
platform does not infer a disruption from that descriptive average.

South Asia incident indicators likewise display reporting counts with their
observation windows. No validated local data support warning/trigger cutoffs,
so those indicators do not answer a PIR or assign an alert tier automatically.
Legacy theaters retain their earlier rules; their calibration limits remain
documented in the decision record.
