import { axiosInstance } from '#services/axiosInstance';
import { validationError } from '#utils/errors';
import infoExtract from '../info.extract.js';

export default async function randomAnimeInfoHandler() {
  const result = await axiosInstance(`/random`);
  if (!result.success) {
    throw new validationError(result.message, 'something went wrong');
  }
  const response = infoExtract(result.data);
  return response;
}
