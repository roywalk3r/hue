import { createRouter } from '../lib/create-app.js';

import * as home from '../modules/home/index.js';
import * as spotlight from '../modules/spotlight/index.js';
import * as topTen from '../modules/topTen/index.js';
import * as animeInfo from '../modules/info/animeInfo/index.js';
import * as randomAnimeInfo from '../modules/info/randomAnimeInfo/index.js';
import * as explore from '../modules/explore/index.js';
import * as search from '../modules/explore/search/index.js';
import * as suggestion from '../modules/suggestion/index.js';
import * as characters from '../modules/characters/index.js';
import * as animeCharacter from '../modules/characterInfo/animeCharacter/index.js';
import * as voiceActor from '../modules/characterInfo/voiceActor/index.js';
import * as genre from '../modules/explore/genre/index.js';
import * as azList from '../modules/explore/az-list/index.js';
import * as producer from '../modules/explore/producer/index.js';
import * as filter from '../modules/explore/filter/index.js';
import * as episodes from '../modules/episodes/index.js';
import * as servers from '../modules/servers/index.js';
import * as stream from '../modules/stream/index.js';
import * as monthlySchedule from '../modules/schedule/monthlySchedule/index.js';
import * as nextEpSchedule from '../modules/schedule/nextEpSchedule/index.js';
import * as meta from '../modules/meta/index.js';
import withTryCatch from '#utils/withTryCatch';

const router = createRouter();

const routes = [
  home,
  spotlight,
  topTen,
  animeInfo,
  randomAnimeInfo,
  search,
  suggestion,
  characters,
  animeCharacter,
  voiceActor,
  genre,
  azList,
  producer,
  filter,
  episodes,
  servers,
  stream,
  monthlySchedule,
  nextEpSchedule,
  meta,
  explore,
];

routes.forEach((route) => {
  router.openapi(route.schema, withTryCatch(route.handler));
});

export default router;
