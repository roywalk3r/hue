import { axiosInstance } from '#services/axiosInstance';
import { validationError } from '#utils/errors';
import spotlightExtract from './spotlight.extract.js';

export default async function spotlightHandler() {
  const result = await axiosInstance('/home');
  if (!result.success) {
    throw new validationError(result.message);
  }
  const response = spotlightExtract(result.data);
  return response;
}
