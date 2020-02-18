import { NgModule } from '@angular/core';
import { RouterModule } from '@angular/router';
import { RustorrentSharedModule } from 'app/shared/shared.module';

import { TrackerComponent } from './tracker.component';

import { trackerRoute } from './tracker.route';

@NgModule({
  imports: [RustorrentSharedModule, RouterModule.forChild([trackerRoute])],
  declarations: [TrackerComponent]
})
export class TrackerModule {}
