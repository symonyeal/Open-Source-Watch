import { test } from 'node:test';
import assert from 'node:assert/strict';
import { acceptsGoogleNewsTitle } from '../src/lib/regionalNews.ts';

const southAsia = {
  filterGoogleNews: true,
  newsRelevanceKeywords: new RegExp(
    'south asia|bangladesh|bangladeshi|\\bindia\\b|\\bindian\\b|indian ocean|pakistan|sri lanka|sri lankan|ceylon|myanmar|burma|burmese|nepal|nepali|nepalese|bhutan|bhutanese|maldives|maldivian|dhaka|chattogram|chittagong|karachi|gwadar|colombo|hambantota|yangon|thilawa|kathmandu|thimphu|mumbai|mundra|chennai|bay of bengal',
    'i',
  ),
};

test('general South Asia queries reject known off-region results', () => {
  assert.equal(acceptsGoogleNewsTitle(
    'Thailand imposes martial law in southern province after attack kills 5 paramilitary troops',
    southAsia,
  ), false);
  assert.equal(acceptsGoogleNewsTitle(
    'Paris extends nighttime curfew for third day amid protests and unrest',
    southAsia,
  ), false);
  assert.equal(acceptsGoogleNewsTitle(
    'Bangladesh port closure disrupts shipping after cyclone', southAsia,
  ), true);
});

test('country queries require that country name or alias in the title', () => {
  assert.equal(acceptsGoogleNewsTitle(
    'Typhoon Champi batters Taiwan', southAsia, 'India',
  ), false);
  assert.equal(acceptsGoogleNewsTitle(
    'Bangladesh Bank website remains offline for third consecutive day', southAsia, 'India',
  ), false);
  assert.equal(acceptsGoogleNewsTitle(
    'Burmese forces report clashes near a regional port', southAsia, 'Myanmar',
  ), true);
  assert.equal(acceptsGoogleNewsTitle(
    'Ceylon port disruption after a storm', southAsia, 'Sri Lanka',
  ), true);
});

test('flagged South Asia results suppress cricket and film noise', () => {
  assert.equal(acceptsGoogleNewsTitle(
    'Kartik Aaryan’s “Captain India” Set Amid Houthi Conflict in Yemen (Exclusive)',
    southAsia, 'India',
  ), false);
  assert.equal(acceptsGoogleNewsTitle(
    'India cricket team wins a late test match', southAsia, 'India',
  ), false);
  assert.equal(acceptsGoogleNewsTitle(
    'India Orders Rooftop Solar Disconnection Following Nationwide Blackout',
    southAsia, 'India',
  ), true);
});

test('other theaters keep the previous unfiltered Google News behavior', () => {
  assert.equal(acceptsGoogleNewsTitle(
    'Paris extends nighttime curfew for third day amid protests and unrest',
    { ...southAsia, filterGoogleNews: false },
  ), true);
});
