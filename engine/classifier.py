"""Kind classifier for text observations (spec P3): TF-IDF (1-2-grams, sublinear
tf) and multinomial logistic regression (C = 4, balanced class weights), trained
on docs/labels/kinds-sample.jsonl, a stratified sample of 200 live observations
labelled by hand against the ACLED definitions (docs/DECISIONS.md).

`compare` is the measurement behind the choice: 3-fold stratified cross-validation,
with each item weighted by its stratum's population share (the sample is stratified
by the rules' own output), against the keyword rules on the same items.
"""
import json
from functools import cache
from pathlib import Path

import numpy as np

LABELS = Path(__file__).resolve().parent.parent / "docs" / "labels"
C = 4.0


def sample():
    X = [json.loads(l) for l in (LABELS / "kinds-sample.jsonl").read_text(encoding="utf8")
         .splitlines()]
    pop = json.loads((LABELS / "kinds-sample.meta.json").read_text(encoding="utf8"))["population"]
    n = {k: sum(x["rule"] == k for x in X) for k in pop}
    w = np.array([pop[x["rule"]] / n[x["rule"]] for x in X])
    return [x["text"] for x in X], np.array([x["label"] for x in X]), w


def pipeline():
    from sklearn.feature_extraction.text import TfidfVectorizer
    from sklearn.linear_model import LogisticRegression
    from sklearn.pipeline import make_pipeline
    return make_pipeline(TfidfVectorizer(ngram_range=(1, 2), sublinear_tf=True),
                         LogisticRegression(C=C, class_weight="balanced", max_iter=5000))


@cache
def model():
    T, y, _ = sample()
    return pipeline().fit(T, y)


def predict(T):
    """Kinds of the texts T."""
    return list(model().predict(T)) if T else []


def compare(seed=0):
    """(rules, classifier) population-weighted accuracy under 3-fold CV."""
    from sklearn.model_selection import StratifiedKFold, cross_val_predict

    from engine.process import rules
    T, y, w = sample()
    p = cross_val_predict(pipeline(), T, y, cv=StratifiedKFold(3, shuffle=True,
                                                                random_state=seed))
    r = np.array([rules(t) for t in T])
    acc = lambda q: float((w * (q == y)).sum() / w.sum())
    return acc(r), acc(p)
