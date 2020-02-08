import { NgModule } from '@angular/core';
import { BrowserModule } from '@angular/platform-browser';

import './vendor';
import { TestMonolitic01SharedModule } from 'app/shared/shared.module';
import { TestMonolitic01CoreModule } from 'app/core/core.module';
import { TestMonolitic01AppRoutingModule } from './app-routing.module';
import { TestMonolitic01HomeModule } from './home/home.module';
import { TestMonolitic01EntityModule } from './entities/entity.module';
// jhipster-needle-angular-add-module-import JHipster will add new module here
import { MainComponent } from './layouts/main/main.component';
import { NavbarComponent } from './layouts/navbar/navbar.component';
import { FooterComponent } from './layouts/footer/footer.component';
import { PageRibbonComponent } from './layouts/profiles/page-ribbon.component';
import { ActiveMenuDirective } from './layouts/navbar/active-menu.directive';
import { ErrorComponent } from './layouts/error/error.component';

@NgModule({
  imports: [
    BrowserModule,
    TestMonolitic01SharedModule,
    TestMonolitic01CoreModule,
    TestMonolitic01HomeModule,
    // jhipster-needle-angular-add-module JHipster will add new module here
    TestMonolitic01EntityModule,
    TestMonolitic01AppRoutingModule
  ],
  declarations: [MainComponent, NavbarComponent, ErrorComponent, PageRibbonComponent, ActiveMenuDirective, FooterComponent],
  bootstrap: [MainComponent]
})
export class TestMonolitic01AppModule {}
