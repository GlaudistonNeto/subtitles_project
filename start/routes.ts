/*
|--------------------------------------------------------------------------
| Routes file
|--------------------------------------------------------------------------
|
| The routes file is used for defining the HTTP routes.
|
*/

import router from '@adonisjs/core/services/router'
import SubtitlesController from '#controllers/subtitles_controller'

router.on('/').render('pages/home')
router.post('/subtitles', [SubtitlesController, 'store'])
